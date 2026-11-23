//! The track's zero-delay feedback (ZDF/TPT) state-variable filter driven by the Filter lane.
use super::filter_cutoff;
pub use crate::eq::CutSlope as FilterSlope;
use crate::master::dsp::{flush, pole_at};
use std::f64::consts::PI;

/// Damping at 0% resonance (`k = 1.0`, slight musical bump matching baseline).
const FILTER_DAMPING: f64 = 1.0;
const FILTER_FADE_SECONDS: f32 = 0.010;
const CUTOFF_SMOOTH_SECONDS: f64 = 0.003;
const FILTER_LOW_OPEN_HZ: f64 = 20_000.0;
const FILTER_HIGH_REST_HZ: f64 = 20.0;

#[derive(Clone, Copy, Debug, Default)]
struct Stage {
    state: [f64; 4],
}

impl Stage {
    #[inline]
    fn run_tpt(&mut self, channel: usize, v0: f64, k: f64, g: f64, nonlinear: bool) -> (f64, f64) {
        let a1 = 1.0 / (1.0 + g * (g + k));
        let a2 = g * a1;
        let a3 = g * a2;
        let (ic1, ic2) = (self.state[channel * 2], self.state[channel * 2 + 1]);
        let fb = if nonlinear {
            // Soft feedback limiting keeps high-resonance 24 dB sweeps stable.
            (ic1 * 0.25).tanh() * 4.0
        } else {
            ic1
        };
        let v3 = v0 - ic2;
        let v1 = a1 * fb + a2 * v3;
        let v2 = ic2 + a2 * fb + a3 * v3;
        self.state[channel * 2] = flush(2.0 * v1 - ic1);
        self.state[channel * 2 + 1] = flush(2.0 * v2 - ic2);
        (v1, v2)
    }
}

#[derive(Clone, Copy, Debug, Default)]
struct Section {
    stages: [Stage; 2],
    target_hz: f64,
    target_log: f64,
    cutoff_hz: Option<f64>,
    g: f64,
    settled: bool,
}

impl Section {
    fn tune(&mut self, target_hz: f64, sample_rate: f64, rate_changed: bool) {
        let target_hz = target_hz.min(sample_rate * 0.45).max(10.0);
        if self.target_hz == target_hz && !rate_changed {
            return;
        }
        self.target_hz = target_hz;
        self.target_log = self.target_hz.ln();
        self.settled = false;
    }

    fn step_cutoff(&mut self, sample_rate: f64, pole_k: f64) -> f64 {
        if !self.settled {
            // Smooth in log-frequency space so fast octave jumps glide evenly.
            let cutoff = self.cutoff_hz.map_or(self.target_hz, |previous| {
                (self.target_log + (previous.ln() - self.target_log) * pole_k).exp()
            });
            self.settled = self.cutoff_hz == Some(cutoff);
            self.cutoff_hz = Some(cutoff);
            self.g = (PI * cutoff / sample_rate).tan();
        }
        self.g
    }

    #[inline]
    fn run_lp(
        &mut self,
        channel: usize,
        x: f64,
        k: f64,
        g: f64,
        slope: FilterSlope,
        res: f64,
    ) -> f64 {
        let nonlin = res > 0.01;
        let (_, lp1) = self.stages[0].run_tpt(channel, x, k, g, nonlin);
        match slope {
            FilterSlope::Slope12 => lp1,
            FilterSlope::Slope24 => self.stages[1].run_tpt(channel, lp1, k, g, nonlin).1,
        }
    }

    #[inline]
    fn run_hp(
        &mut self,
        channel: usize,
        x: f64,
        k: f64,
        g: f64,
        slope: FilterSlope,
        res: f64,
    ) -> f64 {
        let nonlin = res > 0.01;
        let (bp1, lp1) = self.stages[0].run_tpt(channel, x, k, g, nonlin);
        let hp1 = x - k * bp1 - lp1;
        match slope {
            FilterSlope::Slope12 => hp1,
            FilterSlope::Slope24 => {
                let (bp2, lp2) = self.stages[1].run_tpt(channel, hp1, k, g, nonlin);
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

    fn aim(&mut self, source: usize, fade_frames: u32) {
        match self.target {
            None => {
                self.from = [0.0; 3];
                self.from[source] = 1.0;
                self.elapsed = fade_frames;
            }
            Some(target) if target != source => {
                self.from = self.weights(fade_frames);
                self.elapsed = 0;
            }
            Some(_) => {}
        }
        self.target = Some(source);
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
}

fn damping_for(resonance: f32, slope: FilterSlope) -> (f64, f64) {
    let res = f64::from(resonance.clamp(0.0, 100.0) / 100.0);
    let min_k = match slope {
        FilterSlope::Slope12 => 0.16,
        FilterSlope::Slope24 => 0.35,
    };
    (FILTER_DAMPING - res * (FILTER_DAMPING - min_k), res)
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
        }
        let rate = f64::from(sample_rate);
        let cutoff = f64::from(filter_cutoff(value));
        self.low.tune(
            if value < 0.0 {
                cutoff
            } else {
                FILTER_LOW_OPEN_HZ
            },
            rate,
            rate_changed,
        );
        self.high.tune(
            if value > 0.0 {
                cutoff
            } else {
                FILTER_HIGH_REST_HZ
            },
            rate,
            rate_changed,
        );
        self.setting = Some((value, sample_rate));
    }

    /// Filter stereo frames with default settings (`12 dB`, `0%` resonance).
    #[cfg(test)]
    pub(crate) fn process(&mut self, frames: &mut [f32], value: f32, sample_rate: f32) {
        self.process_with(frames, value, sample_rate, 0.0, FilterSlope::Slope12);
    }

    /// Filter stereo frames at `value` (`-1..=1`), `resonance` (`0..=100%`), and `slope`.
    pub(crate) fn process_with(
        &mut self,
        frames: &mut [f32],
        value: f32,
        sample_rate: f32,
        resonance: f32,
        slope: FilterSlope,
    ) {
        self.tune(value, sample_rate);
        let rate = f64::from(sample_rate);
        let fade_frames = self.fade_frames;
        let (k, res) = damping_for(resonance, slope);
        let source = if value < 0.0 {
            0
        } else if value > 0.0 {
            2
        } else {
            1
        };
        self.blend.aim(source, fade_frames);
        for frame in frames.as_chunks_mut::<2>().0 {
            let g_lp = self.low.step_cutoff(rate, self.smooth_pole);
            let g_hp = self.high.step_cutoff(rate, self.smooth_pole);
            let fading = self.blend.elapsed < fade_frames;
            if fading {
                self.blend.elapsed += 1;
            }
            let weights = self.blend.weights(fade_frames);
            for (channel, sample) in frame.iter_mut().enumerate() {
                let v0 = f64::from(*sample);
                let low = self.low.run_lp(channel, v0, k, g_lp, slope, res) as f32;
                let high = self.high.run_hp(channel, v0, k, g_hp, slope, res) as f32;
                let outputs = [low, *sample, high];
                *sample = if fading {
                    weights[0] * outputs[0] + weights[1] * outputs[1] + weights[2] * outputs[2]
                } else {
                    outputs[source]
                };
            }
        }
    }
}

#[cfg(test)]
mod tests;
