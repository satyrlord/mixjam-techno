//! The track's state-variable filter that the Filter lane moves.
use super::filter_cutoff;
use crate::master::dsp::flush32;

/// A track's state-variable filter. Each render reader owns its history.
///
/// A low-pass and a high-pass section both run every frame, so each already
/// follows the signal when the lane crosses to its side. The output moves
/// between the low-pass, the dry input, and the high-pass over a short fade,
/// so crossing zero never clicks.
#[derive(Clone, Copy, Debug, Default)]
pub(crate) struct Filter {
    setting: Option<(f32, f32)>,
    low: Section,
    high: Section,
    blend: Blend,
}

/// The filter's resonance: a slight peak at the cutoff.
const FILTER_DAMPING: f32 = 1.0;

/// The fade between the low-pass, dry, and high-pass outputs.
const FILTER_FADE_SECONDS: f32 = 0.010;

/// The low-pass cutoff while the lane is not below zero.
const FILTER_LOW_OPEN_HZ: f32 = 20_000.0;

/// The high-pass cutoff while the lane is not above zero.
const FILTER_HIGH_REST_HZ: f32 = 20.0;

/// One state-variable filter with its coefficients and per-channel history.
#[derive(Clone, Copy, Debug, Default)]
struct Section {
    state: [f32; 4],
    tuning: Option<(f32, f32)>,
    coefficients: [f32; 3],
}

impl Section {
    fn tune(&mut self, cutoff: f32, sample_rate: f32) {
        if self.tuning != Some((cutoff, sample_rate)) {
            let k = FILTER_DAMPING;
            let g = (std::f32::consts::PI * cutoff.min(sample_rate * 0.45) / sample_rate).tan();
            let a1 = 1.0 / (1.0 + g * (g + k));
            self.coefficients = [a1, g * a1, g * g * a1];
            self.tuning = Some((cutoff, sample_rate));
        }
    }

    /// One sample of one channel: the band-pass and low-pass outputs.
    fn run(&mut self, channel: usize, v0: f32) -> (f32, f32) {
        let [a1, a2, a3] = self.coefficients;
        let (ic1, ic2) = (self.state[channel * 2], self.state[channel * 2 + 1]);
        let v3 = v0 - ic2;
        let v1 = a1 * ic1 + a2 * v3;
        let v2 = ic2 + a2 * ic1 + a3 * v3;
        // Silence decays the history into denormals, which slow the audio thread.
        self.state[channel * 2] = flush32(2.0 * v1 - ic1);
        self.state[channel * 2 + 1] = flush32(2.0 * v2 - ic2);
        (v1, v2)
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

    /// Aim at `source`. The first call sets it at once; later changes fade
    /// from the blend as it stands.
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

impl Filter {
    /// Filter stereo frames in place at one setting. Zero passes them through.
    pub(crate) fn process(&mut self, frames: &mut [f32], value: f32, sample_rate: f32) {
        let k = FILTER_DAMPING;
        let fade_frames = ((FILTER_FADE_SECONDS * sample_rate).round() as u32).max(1);
        if self.setting != Some((value, sample_rate)) {
            self.low.tune(
                if value < 0.0 {
                    filter_cutoff(value)
                } else {
                    FILTER_LOW_OPEN_HZ
                },
                sample_rate,
            );
            self.high.tune(
                if value > 0.0 {
                    filter_cutoff(value)
                } else {
                    FILTER_HIGH_REST_HZ
                },
                sample_rate,
            );
            self.setting = Some((value, sample_rate));
        }
        let source = if value < 0.0 {
            0
        } else if value > 0.0 {
            2
        } else {
            1
        };
        self.blend.aim(source, fade_frames);
        for frame in frames.as_chunks_mut::<2>().0 {
            let fading = self.blend.elapsed < fade_frames;
            if fading {
                self.blend.elapsed += 1;
            }
            let weights = self.blend.weights(fade_frames);
            for (channel, sample) in frame.iter_mut().enumerate() {
                let v0 = *sample;
                let (_, low) = self.low.run(channel, v0);
                let (band, high) = self.high.run(channel, v0);
                let high = v0 - k * band - high;
                let outputs = [low, v0, high];
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
