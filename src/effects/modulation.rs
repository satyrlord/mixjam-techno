//! Stereo BBD-style Chorus and 6-stage ZDF All-Pass Phaser.
use super::{Modulation, ModulationMode, TptPole};
use crate::fx::parts::{Line, bound, warm_sinc_table};
use std::f64::consts::{PI, TAU};

const PHASER_STAGES: usize = 6;
const CHORUS_CENTER_MS: f64 = 7.0;
const CHORUS_SWING_MS: f64 = 4.5;
const PHASER_MIN_HZ: f64 = 180.0;
const PHASER_MAX_HZ: f64 = 3800.0;

#[derive(Clone, Copy, Default)]
struct ApfStage {
    s: f64,
}
impl ApfStage {
    #[inline]
    fn tick(&mut self, x: f64, tan_w: f64) -> f64 {
        let g = tan_w / (1.0 + tan_w);
        let v = (x - self.s) * g;
        let lp = v + self.s;
        let next = lp + v;
        self.s = if next.abs() < 1e-30 { 0.0 } else { next };
        2.0 * lp - x
    }
}

fn lfo_freq(hz: f64, rate: f64, frames: usize, looped: bool) -> f64 {
    if !looped || frames == 0 {
        return hz;
    }
    let cycles = (hz * frames as f64 / rate).round().max(1.0);
    cycles * rate / frames as f64
}

fn modulate(
    frames: &mut [[f64; 2]],
    rate: f64,
    lfo_hz: f64,
    looped: bool,
    mut step: impl FnMut([f64; 2], f64) -> [f64; 2],
) {
    let radians_per_frame = TAU * lfo_hz / rate;
    if looped {
        let warm = (0.2 * rate).ceil() as usize;
        let start = frames.len() - warm % frames.len();
        for i in (start..start + warm).map(|i| i % frames.len()) {
            step(frames[i], radians_per_frame * i as f64);
        }
    }
    for (i, frame) in frames.iter_mut().enumerate() {
        *frame = step(*frame, radians_per_frame * i as f64);
    }
}

pub(super) fn apply(stereo: &mut [[f64; 2]], m: &Modulation, rate: f64, looped: bool) {
    if !m.enabled || m.wet <= 0.0 || stereo.is_empty() {
        return;
    }
    warm_sinc_table();
    let lfo_hz = lfo_freq(f64::from(m.rate_hz), rate, stereo.len(), looped);
    match m.mode {
        ModulationMode::Chorus => run_chorus(stereo, m, rate, lfo_hz, looped),
        ModulationMode::Phaser => run_phaser(stereo, m, rate, lfo_hz, looped),
    }
}

fn run_chorus(stereo: &mut [[f64; 2]], m: &Modulation, rate: f64, lfo_hz: f64, looped: bool) {
    let max_frames =
        ((CHORUS_CENTER_MS + CHORUS_SWING_MS + 4.0) / 1000.0 * rate).ceil() as usize + 16;
    let mut lines = [Line::new(max_frames), Line::new(max_frames)];
    let mut tone = [TptPole::new(9000.0, rate, true); 2];
    let mut fb_prev = [0.0_f64; 2];
    let depth = f64::from(m.depth).clamp(0.0, 1.0);
    let fb = (f64::from(m.feedback) * 0.5).clamp(-0.45, 0.45);
    let wet = f64::from(m.wet).clamp(0.0, 1.0);
    modulate(stereo, rate, lfo_hz, looped, |frame, ph| {
        let mod_l = ph.sin();
        let mod_r = (ph + PI * 0.5).sin();
        let d_l = ((CHORUS_CENTER_MS + CHORUS_SWING_MS * depth * mod_l) / 1000.0 * rate).max(2.0);
        let d_r = ((CHORUS_CENTER_MS + CHORUS_SWING_MS * depth * mod_r) / 1000.0 * rate).max(2.0);
        // Mix two quadrature taps per channel to preserve the mono centre.
        let read_l = lines[0].read_sinc(d_l) * 0.67 + lines[0].read_sinc(d_r) * 0.33;
        let read_r = lines[1].read_sinc(d_r) * 0.67 + lines[1].read_sinc(d_l) * 0.33;
        let damp_l = tone[0].tick(read_l);
        let damp_r = tone[1].tick(read_r);
        lines[0].push(bound(frame[0] + fb_prev[0] * fb) as f32);
        lines[1].push(bound(frame[1] + fb_prev[1] * fb) as f32);
        fb_prev = [damp_l, damp_r];
        [
            frame[0] + (damp_l - frame[0]) * wet,
            frame[1] + (damp_r - frame[1]) * wet,
        ]
    });
}

fn run_phaser(stereo: &mut [[f64; 2]], m: &Modulation, rate: f64, lfo_hz: f64, looped: bool) {
    let mut stages = [[ApfStage::default(); PHASER_STAGES]; 2];
    let mut fb_state = [0.0_f64; 2];
    let depth = f64::from(m.depth).clamp(0.0, 1.0);
    // Give the middle of the regeneration dial a pronounced sweep and
    // approach the stable feedback limit smoothly at either end.
    let feedback_range = 0.85;
    let position = (f64::from(m.feedback) / feedback_range).clamp(-1.0, 1.0);
    let fb = feedback_range * (position * PI * 0.5).sin();
    let wet = f64::from(m.wet).clamp(0.0, 1.0);
    let log_ratio = (PHASER_MAX_HZ / PHASER_MIN_HZ).ln();
    modulate(stereo, rate, lfo_hz, looped, |mut frame, ph| {
        let sweep = [0.5 + 0.5 * ph.sin(), 0.5 + 0.5 * (ph + PI * 0.5).sin()];
        for ch in 0..2 {
            let norm = 0.5 * (1.0 - depth) + depth * sweep[ch];
            let fc = (PHASER_MIN_HZ * (norm * log_ratio).exp()).min(0.45 * rate);
            let tan_w = (PI * fc / rate).tan();
            let mut x = frame[ch] + bound(fb_state[ch]) * fb;
            for st in &mut stages[ch] {
                x = st.tick(x, tan_w);
            }
            fb_state[ch] = x;
            let allpass = x * (1.0 - fb * fb) - frame[ch] * fb;
            frame[ch] += (allpass - frame[ch]) * wet;
        }
        frame
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn chorus_spreads_a_mono_saw_without_losing_its_centre() {
        let rate = 44100.0;
        let mut frames: Vec<_> = (0..151200)
            .map(|i| [0.4 * ((110.0 * i as f64 / rate).fract() - 0.5); 2])
            .collect();
        let settings = Modulation {
            enabled: true,
            depth: 0.75,
            feedback: 0.5,
            wet: 0.5,
            ..Modulation::default()
        };
        apply(&mut frames, &settings, rate, false);
        let lr: f64 = frames.iter().map(|[l, r]| l * r).sum();
        let energy = [0, 1].map(|ch| frames.iter().map(|f| f[ch] * f[ch]).sum::<f64>());
        let correlation = lr / (energy[0] * energy[1]).sqrt();
        assert!((0.90..0.98).contains(&correlation), "{correlation}");
    }

    #[test]
    fn half_wet_phaser_cancels_the_centre_frequency() {
        let rate = 44100.0;
        let hz = (PHASER_MIN_HZ * PHASER_MAX_HZ).sqrt();
        let mut frames: Vec<_> = (0..4096)
            .map(|i| [(TAU * hz * f64::from(i) / rate).sin(); 2])
            .collect();
        let settings = Modulation {
            mode: ModulationMode::Phaser,
            enabled: true,
            depth: 0.0,
            feedback: 0.0,
            wet: 0.5,
            ..Modulation::default()
        };
        apply(&mut frames, &settings, rate, false);
        assert!(frames[2048..].iter().all(|f| f[0].abs() < 0.001));
    }

    #[test]
    fn a_short_wrapped_chorus_has_settled_at_the_first_frame() {
        let mut frames = [[0.2; 2]; 3];
        let settings = Modulation {
            enabled: true,
            wet: 1.0,
            feedback: 0.0,
            ..Modulation::default()
        };
        apply(&mut frames, &settings, 44100.0, true);
        assert!(frames.iter().flatten().all(|x| (x - 0.2).abs() < 1e-5));
    }
}
