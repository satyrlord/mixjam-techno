//! Fractional-delay stereo / ping-pong Echo with feedback HF damping,
//! subtle loop warmth, Trim / WrapLoop seam pre-roll, and RingOut extension.
use super::{Echo, StereoTailEngine, TailMode, TptPole};
use crate::fx::parts::{Line, bound, warm_sinc_table};

fn echo_pole(tone: f32, rate: f64) -> TptPole {
    let t = f64::from(tone).clamp(0.0, 1.0);
    if t <= 1e-4 {
        return TptPole::new(rate, rate, false);
    }
    // Maps tone 0..1 smoothly down from 16 kHz to 1.4 kHz damping corner.
    let hz = (16_000.0 * (1400.0 / 16_000.0_f64).powf(t)).min(0.48 * rate);
    TptPole::new(hz, rate, true)
}

#[inline]
fn read_line_frac(line: &Line, delay: f64) -> f64 {
    if delay >= 8.0 {
        line.read_sinc(delay)
    } else if delay >= 2.0 {
        line.read(delay)
    } else {
        let whole = delay.floor() as usize;
        let f = delay - whole as f64;
        let a = if whole == 0 {
            0.0
        } else {
            f64::from(line.at(whole))
        };
        let b = f64::from(line.at(whole + 1));
        a + (b - a) * f
    }
}

pub(super) struct EchoEngine {
    lines: [Line; 2],
    poles: [TptPole; 2],
    delay_samples: f64,
    feedback: f64,
    wet: f64,
    ping_pong: bool,
    soft_loop: bool,
}

impl EchoEngine {
    pub(super) fn new(echo: &Echo, rate: f64, bpm: f64, loop_frames: Option<usize>) -> Self {
        warm_sinc_table();
        let delay_samples = (rate * 60.0 / bpm * f64::from(echo.delay_beats)).max(1.0);
        // Whole periods do not change a settled periodic signal. Keep the
        // sinc kernel's full history while removing redundant long delays.
        let delay_samples = match loop_frames {
            Some(n) if n > 0 && delay_samples >= 8.0 => 8.0 + (delay_samples - 8.0) % n as f64,
            _ => delay_samples,
        };
        let cap = (delay_samples.ceil() as usize + 32).max(32);
        Self {
            lines: [Line::new(cap), Line::new(cap)],
            poles: [echo_pole(echo.tone, rate); 2],
            delay_samples,
            feedback: f64::from(echo.feedback).clamp(0.0, 0.95),
            wet: f64::from(echo.wet).clamp(0.0, 1.0),
            ping_pong: echo.ping_pong,
            soft_loop: echo.tone > 0.0 || echo.ping_pong,
        }
    }

    pub(super) fn delay_frames(&self) -> usize {
        self.delay_samples.ceil() as usize
    }

    pub(super) fn warm_passes(&self, frames: usize, tail: TailMode) -> usize {
        if tail != TailMode::WrapLoop || frames == 0 {
            return 0;
        }
        let repeats = (1e-5_f64.ln() / self.feedback.ln()).ceil().max(1.0) as usize;
        ((self.delay_frames() + 8) * repeats).div_ceil(frames)
    }
}

impl StereoTailEngine for EchoEngine {
    #[inline]
    fn step_tail(&mut self, dry: [f64; 2], produce: bool) -> [f64; 2] {
        let delayed = [
            read_line_frac(&self.lines[0], self.delay_samples),
            read_line_frac(&self.lines[1], self.delay_samples),
        ];
        let damped = [
            self.poles[0].tick(delayed[0]),
            self.poles[1].tick(delayed[1]),
        ];
        let [in_l, in_r] = if self.ping_pong {
            let mono = 0.5 * (dry[0] + dry[1]);
            [mono + damped[1] * self.feedback, damped[0] * self.feedback]
        } else {
            [
                dry[0] + damped[0] * self.feedback,
                dry[1] + damped[1] * self.feedback,
            ]
        };
        let push_l = if self.soft_loop { bound(in_l) } else { in_l };
        let push_r = if self.soft_loop { bound(in_r) } else { in_r };
        self.lines[0].push(push_l as f32);
        self.lines[1].push(push_r as f32);
        if !produce {
            return dry;
        }
        [
            dry[0] + (damped[0] - dry[0]) * self.wet,
            dry[1] + (damped[1] - dry[1]) * self.wet,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::audio::Pcm;
    use crate::effects::{EffectsState, process};

    #[test]
    fn a_delay_longer_than_a_wrapped_clip_repeats_at_the_correct_frame() {
        let mut input = Pcm {
            sample_rate: 8000,
            channels: 1,
            samples: vec![0.0; 11],
        };
        input.samples[0] = 0.2;
        let state = EffectsState {
            echo: Echo {
                enabled: true,
                feedback: 0.0,
                wet: 1.0,
                delay_beats: 0.5,
                tone: 0.0,
                ping_pong: false,
            },
            tail_mode: TailMode::WrapLoop,
            ..EffectsState::default()
        };
        let out = process(&input, &state, 120.0).unwrap();
        input.samples.rotate_right(2000 % 11);
        assert_eq!(out.samples, input.samples);
    }
}
