//! Zero-delay-feedback multimode SVF and nonlinear 4-pole ladder filter
//! with 2x high-frequency de-cramping and 16x FIR + ADAA2 input drive.
use super::{Filter, FilterMod, FilterMode, oversample_with_preroll, run_oversampled_16x};
use crate::fx::DistortionShape;
use crate::fx::adaa::{Adaa2State, curve_only};
use crate::fx::oversample::{FirOversampler, HalfBandIir2x};
use std::f64::consts::{PI, SQRT_2};

const MIN_HZ: f64 = 16.0;
const SVF_MIN_K: f64 = 0.08;
const LADDER_MAX_FB: f64 = 3.85;
const DRIVE_GAIN: f64 = 12.0;

#[derive(Clone, Copy)]
struct OnePoleState {
    s: f64,
}
impl OnePoleState {
    const ZERO: Self = Self { s: 0.0 };
    #[inline]
    fn tick(&mut self, x: f64, g: f64) -> f64 {
        let v = (x - self.s) * g;
        let y = (v + self.s).clamp(-16.0, 16.0);
        let next = y + v;
        self.s = if next.abs() < 1e-30 { 0.0 } else { next };
        y
    }
}

#[derive(Clone, Copy)]
pub(super) struct FilterCore {
    ic1: f64,
    ic2: f64,
    ladder: [OnePoleState; 4],
    lp_soft: f64,
    decramp: HalfBandIir2x,
}

impl FilterCore {
    pub(super) const ZERO: Self = Self {
        ic1: 0.0,
        ic2: 0.0,
        ladder: [OnePoleState::ZERO; 4],
        lp_soft: 0.0,
        decramp: HalfBandIir2x::ZERO,
    };

    #[inline]
    fn step_svf(&mut self, x: f64, mode: FilterMode, cutoff: f64, res: f64, rate: f64) -> f64 {
        let g = (PI * cutoff / rate).tan();
        let k = SQRT_2 * (SVF_MIN_K / SQRT_2).powf(res.clamp(0.0, 1.0));
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let v3 = x - self.ic2;
        let v1 = a1 * self.ic1 + a2 * v3;
        let v2 = self.ic2 + a2 * self.ic1 + a3 * v3;
        let next1 = 2.0 * v1 - self.ic1;
        let next2 = 2.0 * v2 - self.ic2;
        self.ic1 = if next1.abs() < 1e-30 { 0.0 } else { next1 };
        self.ic2 = if next2.abs() < 1e-30 { 0.0 } else { next2 };
        match mode {
            FilterMode::Lp12 | FilterMode::Lp24 => v2,
            FilterMode::Bp12 => v1,
            FilterMode::Hp12 => x - k * v1 - v2,
        }
    }

    #[inline]
    fn step_ladder(&mut self, x: f64, g: f64, k: f64) -> f64 {
        let g2 = g * g;
        let g3 = g2 * g;
        let g4 = g2 * g2;
        let sum =
            g3 * self.ladder[0].s + g2 * self.ladder[1].s + g * self.ladder[2].s + self.ladder[3].s;
        let drive_in = x * (1.0 + 0.5 * k);
        let fb = 1.6 * (self.lp_soft / 1.6).tanh();
        let u = (drive_in - k * (1.0 - g) * sum) / (1.0 + k * g4 / (1.0 + 0.25 * fb * fb));
        let u = u.clamp(-12.0, 12.0);
        let s0 = self.ladder[0].tick((u / 1.8).tanh() * 1.8, g);
        let s1 = self.ladder[1].tick(s0, g);
        let s2 = self.ladder[2].tick(s1, g);
        let out = self.ladder[3].tick(s2, g);
        self.lp_soft = out;
        out
    }

    #[inline]
    fn step(&mut self, x: f64, mode: FilterMode, cutoff: f64, res: f64, rate: f64) -> f64 {
        self.step_at(x, mode, cutoff, res, rate, needs_decramp(cutoff, rate))
    }

    /// One SVF step at the base rate, or at twice the rate through the
    /// half-band pair. A modulated cutoff keeps one path for the whole render:
    /// a change of path in the signal is a click.
    #[inline]
    fn step_at(
        &mut self,
        x: f64,
        mode: FilterMode,
        cutoff: f64,
        res: f64,
        rate: f64,
        decramp: bool,
    ) -> f64 {
        if decramp {
            let mut decramp = self.decramp;
            let y = decramp.process(x, |up| self.step_svf(up, mode, cutoff, res, rate * 2.0));
            self.decramp = decramp;
            y
        } else {
            self.step_svf(x, mode, cutoff, res, rate)
        }
    }
}

/// True when the SVF runs at twice the rate to keep its response near Nyquist.
#[inline]
fn needs_decramp(cutoff: f64, rate: f64) -> bool {
    cutoff > 0.16 * rate
}

fn drive_curve(drive: f32) -> impl FnMut(f64) -> f64 {
    let pre = 1.0 + DRIVE_GAIN * f64::from(drive);
    // Leave ~1.9 dB headroom so 16x-bandlimited Gibbs overshoots avoid the 1x safety limiter.
    let norm = 0.75 / curve_only(DistortionShape::Overdrive, pre).max(0.5);
    let mut adaa = Adaa2State::default();
    move |x| {
        if drive > 0.0 {
            adaa.process(DistortionShape::Overdrive, x * pre) * norm
        } else {
            x
        }
    }
}

fn filter_warm_frames(looped: bool, cutoff_hz: f64, filter: &Filter, rate: f64) -> usize {
    if !looped {
        return 0;
    }
    let res = f64::from(filter.resonance);
    let damping = match filter.mode {
        FilterMode::Lp24 => 1.0 - (LADDER_MAX_FB * res).powf(0.25) / SQRT_2,
        _ => 0.5 * SQRT_2 * (SVF_MIN_K / SQRT_2).powf(res),
    };
    let decay_per_frame = std::f64::consts::TAU * cutoff_hz / rate * damping;
    // Include the half-band stages' settling time at high cutoffs.
    let warm_frames = (-1e-7_f64.ln() / decay_per_frame).ceil() as usize;
    warm_frames.max(256)
}

#[inline]
fn modulated_cutoff_res(
    base_hz: f64,
    base_res: f64,
    oct: f32,
    boost: f32,
    rate: f64,
) -> (f64, f64) {
    let hz = (base_hz * 2.0_f64.powf(f64::from(oct).clamp(-6.0, 8.0))).clamp(MIN_HZ, rate * 0.48);
    let res = (base_res + f64::from(boost)).clamp(0.0, 0.98);
    (hz, res)
}

#[inline]
fn env_frame_at(k: usize, pre_start: usize, n: usize, looped: bool) -> usize {
    if looped {
        (pre_start + k) % n
    } else {
        k.min(n - 1)
    }
}

fn apply_svf(
    driven: &mut [f64],
    filter: &Filter,
    rate: f64,
    looped: bool,
    base_hz: f64,
    warm_frames: usize,
) {
    let res = f64::from(filter.resonance).clamp(0.0, 1.0);
    let mut core = FilterCore::ZERO;
    if filter.drive > 0.0 {
        run_oversampled_16x(driven, looped, drive_curve(filter.drive));
    }
    for _ in 0..warm_frames.div_ceil(driven.len()) {
        for &x in driven.iter() {
            core.step(x, filter.mode, base_hz, res, rate);
        }
    }
    for x in driven {
        *x = core.step(*x, filter.mode, base_hz, res, rate);
    }
}

fn apply_svf_modulated(
    driven: &mut [f64],
    filter: &Filter,
    rate: f64,
    looped: bool,
    warm_frames: usize,
    m: FilterMod<'_>,
) {
    let n = driven.len();
    let base_hz = f64::from(filter.cutoff_hz).max(MIN_HZ).min(rate * 0.48);
    let base_res = f64::from(filter.resonance).clamp(0.0, 1.0);
    if filter.drive > 0.0 {
        run_oversampled_16x(driven, looped, drive_curve(filter.drive));
    }
    // The cutoff rises with the octaves, so the highest one picks the path.
    let top = m.cutoff_octaves.iter().copied().fold(f32::MIN, f32::max);
    let decramp = needs_decramp(modulated_cutoff_res(base_hz, 0.0, top, 0.0, rate).0, rate);
    let mut core = FilterCore::ZERO;
    let pre = if looped { warm_frames } else { 0 };
    let pre_start = n - pre % n;
    for k in 0..pre {
        let idx = (pre_start + k) % n;
        let (hz, res) = modulated_cutoff_res(
            base_hz,
            base_res,
            m.cutoff_octaves[idx],
            m.res_boost[idx],
            rate,
        );
        core.step_at(driven[idx], filter.mode, hz, res, rate, decramp);
    }
    for (i, sample) in driven.iter_mut().enumerate() {
        let (hz, res) =
            modulated_cutoff_res(base_hz, base_res, m.cutoff_octaves[i], m.res_boost[i], rate);
        *sample = core.step_at(*sample, filter.mode, hz, res, rate, decramp);
    }
}

fn apply_ladder_modulated(
    driven: &mut [f64],
    filter: &Filter,
    rate: f64,
    looped: bool,
    warm_frames: usize,
    m: FilterMod<'_>,
) {
    let n = driven.len();
    crate::fx::prepare_tables();
    let lat = FirOversampler::LATENCY_16X;
    let half_lat = lat / 2;
    let pre = if looped { warm_frames } else { 0 };
    let pre_start = n - pre % n;
    let base_hz = f64::from(filter.cutoff_hz).max(MIN_HZ).min(rate * 0.48);
    let base_res = f64::from(filter.resonance).clamp(0.0, 1.0);
    let mut os = FirOversampler::new(true);
    let mut core = FilterCore::ZERO;
    let mut drive = drive_curve(filter.drive);
    let src = driven.to_vec();
    let total = pre + n + lat;
    for k in 0..total {
        let idx = env_frame_at(k, pre_start, n, looped);
        let x = if k < pre + n || looped { src[idx] } else { 0.0 };
        let e0 = if looped {
            (idx + n - half_lat % n) % n
        } else {
            k.saturating_sub(half_lat).min(n - 1)
        };
        let e1 = if looped {
            (e0 + 1) % n
        } else {
            (e0 + 1).min(n - 1)
        };
        let (hz0, r0) = modulated_cutoff_res(
            base_hz,
            base_res,
            m.cutoff_octaves[e0],
            m.res_boost[e0],
            rate,
        );
        let (hz1, r1) = modulated_cutoff_res(
            base_hz,
            base_res,
            m.cutoff_octaves[e1],
            m.res_boost[e1],
            rate,
        );
        let t0 = (PI * hz0 / (16.0 * rate)).tan();
        let t1 = (PI * hz1 / (16.0 * rate)).tan();
        let (g0, dg) = (t0 / (1.0 + t0), (t1 / (1.0 + t1) - t0 / (1.0 + t0)) / 16.0);
        let (k0, dk) = (LADDER_MAX_FB * r0, LADDER_MAX_FB * (r1 - r0) / 16.0);
        let mut sub = 0_usize;
        let y = os.process(x, |s| {
            let g = g0 + dg * sub as f64;
            let fb = k0 + dk * sub as f64;
            sub += 1;
            core.step_ladder(drive(s), g, fb)
        });
        if k >= pre + lat {
            driven[k - pre - lat] = y;
        }
    }
}

fn apply_ladder_unmodulated(
    driven: &mut [f64],
    filter: &Filter,
    rate: f64,
    looped: bool,
    base_hz: f64,
    warm_frames: usize,
) {
    let mut core = FilterCore::ZERO;
    let res = f64::from(filter.resonance).clamp(0.0, 1.0);
    let tan_w = (PI * base_hz / (16.0 * rate)).tan();
    let g = tan_w / (1.0 + tan_w);
    let mut drive = drive_curve(filter.drive);
    oversample_with_preroll(driven, looped, warm_frames, |x| {
        core.step_ladder(drive(x), g, LADDER_MAX_FB * res)
    });
}

pub(super) fn apply(
    channel: &mut [f64],
    filter: &Filter,
    rate: f64,
    looped: bool,
    modulation: Option<FilterMod<'_>>,
) {
    if !filter.enabled || filter.wet <= 0.0 || channel.is_empty() {
        return;
    }
    let mut driven = channel.to_vec();
    let base_hz = f64::from(filter.cutoff_hz).max(MIN_HZ).min(rate * 0.48);
    let warm_frames = filter_warm_frames(looped, base_hz, filter, rate);
    match (filter.mode, modulation) {
        (FilterMode::Lp24, Some(m)) => {
            apply_ladder_modulated(&mut driven, filter, rate, looped, warm_frames, m);
        }
        (FilterMode::Lp24, None) => {
            apply_ladder_unmodulated(&mut driven, filter, rate, looped, base_hz, warm_frames);
        }
        (_, Some(m)) => {
            apply_svf_modulated(&mut driven, filter, rate, looped, warm_frames, m);
        }
        (_, None) => {
            apply_svf(&mut driven, filter, rate, looped, base_hz, warm_frames);
        }
    }
    for (dry, y) in channel.iter_mut().zip(driven) {
        *dry += (y - *dry) * f64::from(filter.wet);
    }
}

/// Sample the analytical frequency response (dB over 20 Hz..20 kHz) for UI scopes.
pub fn sample_filter_curve(filter: &Filter, points: usize) -> Vec<f32> {
    if points == 0 {
        return Vec::new();
    }
    let denom = (points.saturating_sub(1)).max(1) as f64;
    let fc = f64::from(filter.cutoff_hz).max(MIN_HZ);
    let q_svf =
        (SQRT_2 * (SVF_MIN_K / SQRT_2).powf(f64::from(filter.resonance).clamp(0.0, 1.0))).recip();
    let k_lad = LADDER_MAX_FB * f64::from(filter.resonance).clamp(0.0, 1.0);
    let wet = if filter.enabled {
        f64::from(filter.wet)
    } else {
        0.0
    };
    (0..points)
        .map(|i| {
            let hz = 20.0 * 1000.0_f64.powf(i as f64 / denom);
            let mixed = analog_response_mag(filter.mode, hz / fc, q_svf, k_lad, wet).max(1e-4);
            (20.0 * mixed.log10()).clamp(-36.0, 24.0) as f32
        })
        .collect()
}

fn analog_response_mag(mode: FilterMode, w: f64, q: f64, k_lad: f64, wet: f64) -> f64 {
    let w2 = w * w;
    let re = 1.0 - w2;
    let im = w / q;
    let (den_re, den_im, num_re, num_im) = match mode {
        FilterMode::Lp12 => (re, im, 1.0, 0.0),
        FilterMode::Hp12 => (re, im, -w2, 0.0),
        FilterMode::Bp12 => (re, im, 0.0, w),
        FilterMode::Lp24 => {
            // H(jw) = (1 + 0.5*k) / ((1 + jw)^4 + k)
            let jr = 1.0 - 6.0 * w2 + w2 * w2 + k_lad;
            let ji = 4.0 * w - 4.0 * w2 * w;
            (jr, ji, 1.0 + 0.5 * k_lad, 0.0)
        }
    };
    // Dry and wet share a source. Sum their complex gains before taking
    // the magnitude so the scope includes cancellation and resonance.
    let mixed_re = (1.0 - wet) * den_re + wet * num_re;
    let mixed_im = (1.0 - wet) * den_im + wet * num_im;
    mixed_re.hypot(mixed_im) / den_re.hypot(den_im)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn driven_resonant_ladder_rejects_folded_harmonics() {
        let rate = 44_100.0;
        let phase = std::f64::consts::TAU * 4048.0 / rate;
        let mut tone: Vec<_> = (0..22_306)
            .map(|i| 0.707 * (phase * f64::from(i)).sin())
            .collect();
        let filter = Filter {
            enabled: true,
            mode: FilterMode::Lp24,
            cutoff_hz: 1200.0,
            resonance: 0.85,
            drive: 1.0,
            wet: 1.0,
        };
        apply(&mut tone, &filter, rate, false, None);
        // This quarter second contains whole cycles of the tone and its harmonics.
        let settled = &tone[11_025..22_050];
        let n = settled.len() as f64;
        let mut residual = settled.iter().map(|x| x * x).sum::<f64>() / n;
        for harmonic in 1..=5 {
            let mut cosine = 0.0;
            let mut sine = 0.0;
            for (i, x) in settled.iter().enumerate() {
                let (s, c) = (phase * f64::from(harmonic) * i as f64).sin_cos();
                cosine += c * x;
                sine += s * x;
            }
            let power = 2.0 * (cosine * cosine + sine * sine) / (n * n);
            if harmonic == 1 {
                assert!(power > 1e-5);
            }
            residual -= power;
        }
        assert!(residual < 1e-9, "nonharmonic power: {residual}");
    }

    #[test]
    fn scope_matches_rendered_gain_at_cutoff_for_each_mode_and_wet_mix() {
        let rate = 48_000.0;
        let frames = 4800;
        let input: Vec<f64> = (0..frames)
            .map(|i| 0.001 * (std::f64::consts::TAU * 200.0 * i as f64 / rate).sin())
            .collect();
        for mode in [
            FilterMode::Lp12,
            FilterMode::Bp12,
            FilterMode::Hp12,
            FilterMode::Lp24,
        ] {
            for wet in [0.5, 1.0] {
                let filter = Filter {
                    enabled: true,
                    cutoff_hz: 200.0,
                    resonance: 0.6,
                    wet,
                    mode,
                    ..Filter::default()
                };
                let mut output = input.clone();
                apply(&mut output, &filter, rate, true, None);
                let power = |signal: &[f64]| signal.iter().map(|x| x * x).sum::<f64>();
                let measured = 10.0 * (power(&output) / power(&input)).log10();
                let shown = f64::from(sample_filter_curve(&filter, 4)[1]);
                assert!(
                    (measured - shown).abs() < 0.02,
                    "{mode:?} {wet}: {measured} vs {shown}"
                );
            }
        }
    }

    #[test]
    fn short_wrapped_resonant_filter_reaches_its_periodic_state() {
        let filter = Filter {
            enabled: true,
            mode: FilterMode::Lp12,
            cutoff_hz: 20.0,
            resonance: 1.0,
            drive: 0.0,
            wet: 1.0,
        };
        let mut dc = [0.1];
        apply(&mut dc, &filter, 44_100.0, true, None);
        assert!((dc[0] - 0.1).abs() < 1e-6, "{}", dc[0]);
    }

    #[test]
    fn a_cutoff_sweep_across_the_decramp_threshold_does_not_click() {
        // A 0 to 3 octave ramp from 2 kHz crosses 0.16 of the rate (7,056 Hz)
        // near frame 4,966. A change of the SVF path there was a step in the
        // output: a second difference up to 45 times the local one.
        let rate = 44_100.0;
        let frames = 8192;
        let input: Vec<f64> = (0..frames)
            .map(|i| 0.4 * (std::f64::consts::TAU * 3_000.0 * i as f64 / rate).sin())
            .collect();
        let octs: Vec<f32> = (0..frames)
            .map(|i| 3.0 * i as f32 / frames as f32)
            .collect();
        let res = vec![0.0_f32; frames];
        for mode in [
            FilterMode::Lp12,
            FilterMode::Bp12,
            FilterMode::Hp12,
            FilterMode::Lp24,
        ] {
            let filter = Filter {
                enabled: true,
                mode,
                cutoff_hz: 2000.0,
                resonance: 0.4,
                drive: 0.0,
                wet: 1.0,
            };
            let mut out = input.clone();
            apply(
                &mut out,
                &filter,
                rate,
                false,
                Some(FilterMod {
                    cutoff_octaves: &octs,
                    res_boost: &res,
                }),
            );
            let d2: Vec<f64> = (2..frames)
                .map(|i| (out[i] - 2.0 * out[i - 1] + out[i - 2]).abs())
                .collect();
            // Two cycles of the tone on each side, away from the frame itself.
            for i in 512..d2.len() - 40 {
                let local = d2[i - 40..i - 10]
                    .iter()
                    .chain(&d2[i + 10..i + 40])
                    .fold(0.0_f64, |a, &b| a.max(b));
                assert!(
                    d2[i] <= 1.5 * local + 1e-6,
                    "{mode:?} click at frame {}: {} against {local}",
                    i + 2,
                    d2[i]
                );
            }
        }
    }

    #[test]
    fn modulated_filter_opens_above_nominal_cutoff() {
        let rate = 44_100.0;
        let frames = 4096;
        let input: Vec<f64> = (0..frames)
            .map(|i| 0.4 * (std::f64::consts::TAU * 2_000.0 * i as f64 / rate).sin())
            .collect();
        for mode in [FilterMode::Lp12, FilterMode::Lp24] {
            let filter = Filter {
                enabled: true,
                mode,
                cutoff_hz: 200.0,
                resonance: 0.4,
                drive: 0.0,
                wet: 1.0,
            };
            let mut dark = input.clone();
            apply(&mut dark, &filter, rate, false, None);
            let mut open = input.clone();
            let octs = vec![4.0_f32; frames];
            let res = vec![0.1_f32; frames];
            apply(
                &mut open,
                &filter,
                rate,
                false,
                Some(FilterMod {
                    cutoff_octaves: &octs,
                    res_boost: &res,
                }),
            );
            let rms = |s: &[f64]| (s.iter().map(|x| x * x).sum::<f64>() / s.len() as f64).sqrt();
            assert!(rms(&open[512..]) > rms(&dark[512..]) * 6.0, "{mode:?}");
        }
    }
}
