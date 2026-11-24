//! The track's zero-delay feedback (ZDF/TPT) state-variable filter driven by the Filter lane.
pub use crate::eq::CutSlope as FilterSlope;
use crate::master::dsp::{fast_exp2, fast_tan, pole_at};
use std::f64::consts::{LN_10, LOG2_E, PI};

/// Damping at 0% resonance (`k = 1.0`, slight musical bump matching baseline).
const FILTER_DAMPING: f64 = 1.0;
const FILTER_FADE_SECONDS: f32 = 0.010;
const CUTOFF_SMOOTH_SECONDS: f64 = 0.003;
const LN_20: f64 = 2.995_732_273_553_991;
const LN_20K: f64 = 9.903_487_552_536_127;
const LN_500: f64 = 6.214_608_098_422_191;

#[derive(Clone, Copy, Debug, Default)]
struct Stage {
    state: [f64; 4],
}

impl Stage {
    #[inline]
    fn run_tpt(
        &mut self,
        channel: usize,
        v0: f64,
        coeffs: [f64; 3],
        nonlinear: bool,
    ) -> (f64, f64) {
        let [a1, a2, a3] = coeffs;
        let (ic1, ic2) = (self.state[channel * 2], self.state[channel * 2 + 1]);
        let fb = if nonlinear {
            // Division-free C1 cubic soft feedback limiting keeps sweeps stable.
            let s = (ic1 * 0.25).clamp(-1.5, 1.5);
            (s - (4.0 / 27.0) * s * s * s) * 4.0
        } else {
            ic1
        };
        let v3 = v0 - ic2;
        let v1 = a1 * fb + a2 * v3;
        let v2 = ic2 + a2 * fb + a3 * v3;
        self.state[channel * 2] = 2.0 * v1 - ic1;
        self.state[channel * 2 + 1] = 2.0 * v2 - ic2;
        (v1, v2)
    }

    fn flush_state(&mut self) {
        for s in &mut self.state {
            if s.abs() < 1e-46 {
                *s = 0.0;
            }
        }
    }
}

#[inline]
fn tpt_coeffs(g: f64, k: f64) -> [f64; 3] {
    let a1 = 1.0 / (1.0 + g * (g + k));
    let a2 = g * a1;
    let a3 = g * a2;
    [a1, a2, a3]
}

#[derive(Clone, Copy, Debug, Default)]
struct Section {
    stages: [Stage; 2],
    target_log: f64,
    cutoff_log: Option<f64>,
    g: f64,
    coeffs: [f64; 3],
    last_k: f64,
    settled: bool,
}

impl Section {
    fn clear_stages(&mut self) {
        self.stages = [Stage::default(); 2];
    }

    fn flush_states(&mut self) {
        self.stages[0].flush_state();
        self.stages[1].flush_state();
    }

    fn tune_log(&mut self, log_hz: f64, max_log: f64, force: bool) {
        let target_log = log_hz.clamp(LN_10, max_log);
        if self.target_log == target_log && !force {
            return;
        }
        self.target_log = target_log;
        self.settled = false;
    }

    #[inline]
    fn step_coeffs(&mut self, sample_rate: f64, pole_k: f64, k: f64) -> [f64; 3] {
        if !self.settled || self.last_k != k {
            if !self.settled {
                // Smooth directly in log-frequency space without extra ln()/exp()/tan().
                let next_log = self.cutoff_log.map_or(self.target_log, |previous| {
                    self.target_log + (previous - self.target_log) * pole_k
                });
                self.settled = (next_log - self.target_log).abs() < 1e-9;
                let cutoff_log = if self.settled {
                    self.target_log
                } else {
                    next_log
                };
                self.cutoff_log = Some(cutoff_log);
                let cutoff = fast_exp2(cutoff_log * LOG2_E);
                self.g = fast_tan(PI * cutoff / sample_rate);
            }
            self.last_k = k;
            self.coeffs = tpt_coeffs(self.g, k);
        }
        self.coeffs
    }

    #[inline]
    fn run_lp(
        &mut self,
        channel: usize,
        x: f64,
        coeffs: [f64; 3],
        slope: FilterSlope,
        nonlin: bool,
    ) -> f64 {
        let (_, lp1) = self.stages[0].run_tpt(channel, x, coeffs, nonlin);
        match slope {
            FilterSlope::Slope12 => lp1,
            FilterSlope::Slope24 => self.stages[1].run_tpt(channel, lp1, coeffs, nonlin).1,
        }
    }

    #[inline]
    fn run_hp(
        &mut self,
        channel: usize,
        x: f64,
        k: f64,
        coeffs: [f64; 3],
        slope: FilterSlope,
        nonlin: bool,
    ) -> f64 {
        let (bp1, lp1) = self.stages[0].run_tpt(channel, x, coeffs, nonlin);
        let hp1 = x - k * bp1 - lp1;
        match slope {
            FilterSlope::Slope12 => hp1,
            FilterSlope::Slope24 => {
                let (bp2, lp2) = self.stages[1].run_tpt(channel, hp1, coeffs, nonlin);
                hp1 - k * bp2 - lp2
            }
        }
    }
}

/// Which output the filter sounds: 0 low-pass, 1 dry, 2 high-pass.
#[derive(Clone, Copy, Debug, Default)]
struct Blend {
    target: Option<usize>,
    from: [f32; 3],
    elapsed: u32,
}

impl Blend {
    fn weights(&self, fade_frames: u32) -> [f32; 3] {
        let Some(target) = self.target else {
            return [0.0; 3];
        };
        let t = (self.elapsed as f32 / fade_frames as f32).min(1.0);
        let mut weights = self.from.map(|from| from * (1.0 - t));
        weights[target] += t;
        weights
    }

    fn aim(&mut self, source: usize, fade_frames: u32) -> Option<usize> {
        let mut entering = None;
        match self.target {
            None => {
                self.from = [0.0; 3];
                self.from[source] = 1.0;
                self.elapsed = fade_frames;
            }
            Some(target) if target != source => {
                self.from = self.weights(fade_frames);
                self.elapsed = 0;
                entering = Some(source);
            }
            Some(_) => {}
        }
        self.target = Some(source);
        entering
    }
}

/// A track's ZDF/TPT state-variable filter with separate low-pass and high-pass cores.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Filter {
    low: Section,
    high: Section,
    blend: Blend,
    setting: Option<(f32, f32)>,
    fade_frames: u32,
    smooth_pole: f64,
    smooth_pole_8: f64,
    max_log: f64,
}

fn damping_for(resonance: f32, slope: FilterSlope) -> (f64, bool) {
    let res = f64::from(resonance.clamp(0.0, 100.0) / 100.0);
    let min_k = match slope {
        FilterSlope::Slope12 => 0.16,
        FilterSlope::Slope24 => 0.35,
    };
    (FILTER_DAMPING - res * (FILTER_DAMPING - min_k), res > 0.01)
}

impl Filter {
    fn tune(&mut self, value: f32, sample_rate: f32) {
        if self.setting == Some((value, sample_rate)) {
            return;
        }
        let rate_changed = self.setting.is_none_or(|(_, rate)| rate != sample_rate);
        if rate_changed {
            self.fade_frames = ((FILTER_FADE_SECONDS * sample_rate).round() as u32).max(1);
            self.smooth_pole = pole_at(CUTOFF_SMOOTH_SECONDS, f64::from(sample_rate));
            self.smooth_pole_8 = pole_at(CUTOFF_SMOOTH_SECONDS, f64::from(sample_rate) / 8.0);
            self.max_log = (f64::from(sample_rate) * 0.45).ln();
        }
        let v = f64::from(value);
        self.low.tune_log(
            if v < 0.0 { LN_20K + v * LN_500 } else { LN_20K },
            self.max_log,
            rate_changed,
        );
        self.high.tune_log(
            if v > 0.0 { LN_20 + v * LN_500 } else { LN_20 },
            self.max_log,
            rate_changed,
        );
        self.setting = Some((value, sample_rate));
    }

    fn refresh_band_coeffs(
        band: &mut Section,
        active: bool,
        refresh: bool,
        rate: f64,
        pole_k: f64,
        k: f64,
    ) -> [f64; 3] {
        if active && (refresh || band.cutoff_log.is_none() || band.last_k != k) {
            band.step_coeffs(rate, pole_k, k)
        } else {
            band.coeffs
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn process_steady_slice(
        &mut self,
        slice: &mut [[f32; 2]],
        source: usize,
        k: f64,
        c_lp: [f64; 3],
        c_hp: [f64; 3],
        slope: FilterSlope,
        nonlin: bool,
    ) {
        match source {
            0 => {
                for frame in slice {
                    frame[0] = self.low.run_lp(0, f64::from(frame[0]), c_lp, slope, nonlin) as f32;
                    frame[1] = self.low.run_lp(1, f64::from(frame[1]), c_lp, slope, nonlin) as f32;
                }
            }
            2 => {
                for frame in slice {
                    frame[0] = self
                        .high
                        .run_hp(0, f64::from(frame[0]), k, c_hp, slope, nonlin)
                        as f32;
                    frame[1] = self
                        .high
                        .run_hp(1, f64::from(frame[1]), k, c_hp, slope, nonlin)
                        as f32;
                }
            }
            _ => {}
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn step_fading_frame(
        &mut self,
        frame: &mut [f32; 2],
        weights: [f32; 3],
        k: f64,
        c_lp: [f64; 3],
        c_hp: [f64; 3],
        slope: FilterSlope,
        nonlin: bool,
    ) {
        let run_lp = weights[0] > 0.0;
        let run_hp = weights[2] > 0.0;
        for (channel, sample) in frame.iter_mut().enumerate() {
            let v0 = f64::from(*sample);
            let low = if run_lp {
                self.low.run_lp(channel, v0, c_lp, slope, nonlin) as f32
            } else {
                0.0
            };
            let high = if run_hp {
                self.high.run_hp(channel, v0, k, c_hp, slope, nonlin) as f32
            } else {
                0.0
            };
            *sample = weights[0] * low + weights[1] * *sample + weights[2] * high;
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn step_frame_filter(
        &mut self,
        frame: &mut [f32; 2],
        source: usize,
        k: f64,
        nonlin: bool,
        slope: FilterSlope,
        rate: f64,
        pole_k: f64,
        step_cutoff_now: bool,
    ) {
        let fade_frames = self.fade_frames;
        let fading = self.blend.elapsed < fade_frames;
        if fading {
            self.blend.elapsed += 1;
        }
        let weights = self.blend.weights(fade_frames);
        let c_lp = Self::refresh_band_coeffs(
            &mut self.low,
            source == 0 || (fading && weights[0] > 0.0),
            step_cutoff_now,
            rate,
            pole_k,
            k,
        );
        let c_hp = Self::refresh_band_coeffs(
            &mut self.high,
            source == 2 || (fading && weights[2] > 0.0),
            step_cutoff_now,
            rate,
            pole_k,
            k,
        );
        if !fading {
            self.process_steady_slice(
                std::slice::from_mut(frame),
                source,
                k,
                c_lp,
                c_hp,
                slope,
                nonlin,
            );
        } else {
            self.step_fading_frame(frame, weights, k, c_lp, c_hp, slope, nonlin);
        }
    }

    fn prepare_aim(&mut self, value: f32, sample_rate: f32) -> usize {
        self.tune(value, sample_rate);
        let source = if value < 0.0 {
            0
        } else if value > 0.0 {
            2
        } else {
            1
        };
        let entering = self.blend.aim(source, self.fade_frames);
        if entering == Some(0) && self.blend.from[0] == 0.0 {
            self.low.clear_stages();
        } else if entering == Some(2) && self.blend.from[2] == 0.0 {
            self.high.clear_stages();
        }
        source
    }

    /// Filter stereo frames with default settings (`12 dB`, `0%` resonance).
    #[cfg(test)]
    pub(crate) fn process(&mut self, frames: &mut [f32], value: f32, sample_rate: f32) {
        self.process_with(frames, value, sample_rate, 0.0, FilterSlope::Slope12);
    }

    /// Filter stereo frames at `value` (`-1..=1`), `resonance` (`0..=100%`), and `slope`.
    #[cfg(test)]
    pub(crate) fn process_with(
        &mut self,
        frames: &mut [f32],
        value: f32,
        sample_rate: f32,
        resonance: f32,
        slope: FilterSlope,
    ) {
        let rate = f64::from(sample_rate);
        let (k, nonlin) = damping_for(resonance, slope);
        let source = self.prepare_aim(value, sample_rate);
        let pole_k = self.smooth_pole;
        for frame in frames.as_chunks_mut::<2>().0 {
            self.step_frame_filter(frame, source, k, nonlin, slope, rate, pole_k, true);
        }
        self.low.flush_states();
        self.high.flush_states();
    }

    /// Filter an interleaved block driven by `envelope` using bit-exact
    /// 8-frame aligned cutoff steps and branchless chunk inner loops.
    pub(crate) fn process_lane(
        &mut self,
        frames: &mut [f32],
        start_frame: u64,
        envelope: &super::Envelope,
        sample_rate: f32,
        resonance: f32,
        slope: FilterSlope,
    ) {
        let rate = f64::from(sample_rate);
        let (k, nonlin) = damping_for(resonance, slope);
        let mut source = self.blend.target.unwrap_or(1);
        let mut offset = 0;
        let total = frames.len() / 2;
        while offset < total {
            let pos = start_frame + offset as u64;
            let boundary = (pos & 7) == 0 || self.setting.is_none();
            if boundary {
                source = self.prepare_aim(envelope.at(pos), sample_rate);
            }
            let fade_frames = self.fade_frames;
            let pole_k = self.smooth_pole_8;
            let chunk = (8 - ((pos & 7) as usize)).min(total - offset);
            let slice = &mut frames[offset * 2..(offset + chunk) * 2]
                .as_chunks_mut::<2>()
                .0;
            let fading = self.blend.elapsed < fade_frames;
            let c_lp = Self::refresh_band_coeffs(
                &mut self.low,
                source == 0 || fading,
                boundary,
                rate,
                pole_k,
                k,
            );
            let c_hp = Self::refresh_band_coeffs(
                &mut self.high,
                source == 2 || fading,
                boundary,
                rate,
                pole_k,
                k,
            );
            if !fading {
                self.process_steady_slice(slice, source, k, c_lp, c_hp, slope, nonlin);
            } else {
                for frame in slice.iter_mut() {
                    self.step_frame_filter(frame, source, k, nonlin, slope, rate, pole_k, false);
                }
            }
            offset += chunk;
        }
        self.low.flush_states();
        self.high.flush_states();
    }
}

#[cfg(test)]
mod tests;
