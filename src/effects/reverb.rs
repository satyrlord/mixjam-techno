//! Compact 16-line signed-permuted Sylvester Hadamard FDN Reverb reusing
//! `fx/reverb` topologies, adapted to arbitrary source sample rates and TailMode.
use super::{Reverb, ReverbSpace, StereoTailEngine, TailMode, TptPole};
use crate::fx::hadamard;
use crate::fx::parts::{Line, bound};
use std::f64::consts::TAU;

const LINES: usize = 16;
const PERM: [usize; LINES] = [3, 10, 5, 14, 1, 12, 7, 0, 11, 2, 15, 6, 9, 4, 13, 8];
const PSIGN: [f64; LINES] = [
    1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0, 1.0, -1.0,
];
const SIGN_IN: [f64; 8] = [1.0, -1.0, 1.0, -1.0, 1.0, -1.0, 1.0, -1.0];
const SIGN_OUT: [[f64; 8]; 2] = [
    [1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0, -1.0],
    [1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, -1.0],
];
const LINE_SHARE: f64 = 0.353_553_390_593_273_8;

fn base_ms(space: ReverbSpace) -> [f64; LINES] {
    match space {
        ReverbSpace::Room => [
            11.3, 12.4, 13.7, 15.1, 16.6, 18.3, 20.1, 22.2, 24.4, 26.9, 29.7, 32.7, 36.0, 39.6,
            43.6, 47.9,
        ],
        ReverbSpace::Hall => [
            29.7, 32.5, 35.6, 39.0, 42.6, 46.7, 51.1, 55.9, 61.2, 67.0, 73.3, 80.2, 87.8, 96.1,
            105.2, 113.4,
        ],
        ReverbSpace::Plate => [
            7.1, 8.2, 9.5, 10.9, 12.6, 14.6, 16.8, 19.4, 22.5, 25.9, 30.0, 34.6, 40.0, 46.2, 53.5,
            61.9,
        ],
    }
}

fn damp_pole(damping: f32, rate: f64) -> TptPole {
    let t = f64::from(damping).clamp(0.0, 1.0);
    if t <= 1e-4 {
        return TptPole::new(rate, rate, false);
    }
    let hz = (15_000.0 * (1200.0 / 15_000.0_f64).powf(t)).min(0.45 * rate);
    TptPole::new(hz, rate, true)
}

struct AllPassDelay {
    line: Line,
    delay: usize,
    gain: f64,
}
impl AllPassDelay {
    fn new(ms: f64, rate: f64, gain: f64) -> Self {
        let delay = ((ms / 1000.0 * rate).round() as usize).max(2);
        Self {
            line: Line::new(delay + 8),
            delay,
            gain,
        }
    }
    #[inline]
    fn tick(&mut self, x: f64) -> f64 {
        let delayed = f64::from(self.line.at(self.delay));
        let v = x + delayed * self.gain;
        self.line.push(v as f32);
        delayed - self.gain * v
    }
}

pub(super) struct ReverbEngine {
    diff: [[AllPassDelay; 2]; 2],
    lines: Vec<Line>,
    lengths: [f64; LINES],
    gains: [f64; LINES],
    poles: [TptPole; LINES],
    wet: f64,
    lfo_phase: f64,
    lfo_step: f64,
}

impl ReverbEngine {
    pub(super) fn new(reverb: &Reverb, rate: f64, loop_frames: Option<usize>) -> Self {
        let ms_list = base_ms(reverb.space);
        let decay = f64::from(reverb.decay_s).clamp(0.2, 8.0);
        let mut lines = Vec::with_capacity(LINES);
        let mut lengths = [0.0; LINES];
        let mut gains = [0.0; LINES];
        for (i, &ms) in ms_list.iter().enumerate() {
            let frames = ((ms / 1000.0 * rate).round() as usize).max(4);
            lines.push(Line::new(frames + 16));
            lengths[i] = frames as f64;
            gains[i] = 10.0_f64.powf(-3.0 * (frames as f64 / rate) / decay);
        }
        let lfo_hz = match loop_frames {
            Some(n) if n > 0 => ((0.35 * n as f64 / rate).round().max(1.0)) * rate / n as f64,
            _ => 0.35,
        };
        Self {
            diff: [
                [
                    AllPassDelay::new(4.7, rate, 0.55),
                    AllPassDelay::new(9.3, rate, 0.5),
                ],
                [
                    AllPassDelay::new(5.3, rate, 0.55),
                    AllPassDelay::new(10.1, rate, 0.5),
                ],
            ],
            lines,
            lengths,
            gains,
            poles: [damp_pole(reverb.damping, rate); LINES],
            wet: f64::from(reverb.wet).clamp(0.0, 1.0),
            lfo_phase: 0.0,
            lfo_step: TAU * lfo_hz / rate,
        }
    }
}

pub(super) fn warm_passes(frames: usize, decay_s: f32, rate: f64, tail: TailMode) -> usize {
    if tail != TailMode::WrapLoop || frames == 0 {
        return 0;
    }
    let loop_s = frames as f64 / rate;
    (4.0 * f64::from(decay_s) / (3.0 * loop_s)).ceil() as usize
}

impl StereoTailEngine for ReverbEngine {
    #[inline]
    fn step_tail(&mut self, dry: [f64; 2], produce: bool) -> [f64; 2] {
        let in_l0 = self.diff[0][0].tick(dry[0]);
        let in_l = self.diff[0][1].tick(in_l0);
        let in_r0 = self.diff[1][0].tick(dry[1]);
        let in_r = self.diff[1][1].tick(in_r0);
        let mut raw = [0.0_f64; LINES];
        let swing = 1.5;
        let ph_step = TAU / LINES as f64;
        for (i, slot) in raw.iter_mut().enumerate() {
            let mod_d = self.lengths[i] + swing * (self.lfo_phase + i as f64 * ph_step).sin();
            *slot = self.lines[i].read(mod_d.max(2.0));
        }
        self.lfo_phase = (self.lfo_phase + self.lfo_step).rem_euclid(TAU);
        let mut mixed = raw;
        hadamard(&mut mixed);
        for i in 0..LINES {
            let inject = if i % 2 == 0 {
                in_l * SIGN_IN[i / 2]
            } else {
                in_r * SIGN_IN[i / 2]
            } * LINE_SHARE;
            let damped = self.poles[i].tick(mixed[PERM[i]] * PSIGN[i] * self.gains[i]);
            self.lines[i].push(bound(inject + damped) as f32);
        }
        if !produce {
            return dry;
        }
        let (mut wet_l, mut wet_r) = (0.0, 0.0);
        for k in 0..8 {
            wet_l += raw[k * 2] * SIGN_OUT[0][k];
            wet_r += raw[k * 2 + 1] * SIGN_OUT[1][k];
        }
        // Include an opposite-polarity reflection from the other side.
        let gain = LINE_SHARE * 1.4;
        let (wet_l, wet_r) = ((wet_l - 0.2 * wet_r) * gain, (wet_r - 0.2 * wet_l) * gain);
        [
            dry[0] + (wet_l - dry[0]) * self.wet,
            dry[1] + (wet_r - dry[1]) * self.wet,
        ]
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn late_reflections_keep_stereo_width_and_opposite_polarity() {
        let rate = 16000.0;
        for space in [ReverbSpace::Room, ReverbSpace::Hall, ReverbSpace::Plate] {
            let reverb = Reverb {
                enabled: true,
                space,
                wet: 1.0,
                ..Reverb::default()
            };
            let mut engine = ReverbEngine::new(&reverb, rate, None);
            let mut energy = [0.0; 3];
            for i in 0..16000 {
                let dry = if i == 0 { [0.5; 2] } else { [0.0; 2] };
                let [l, r] = engine.step_tail(dry, true);
                if i >= 2400 {
                    energy[0] += l * l;
                    energy[1] += r * r;
                    energy[2] += l * r;
                }
            }
            let corr = energy[2] / (energy[0] * energy[1]).sqrt();
            assert!((-0.7..-0.25).contains(&corr), "{space:?}: {corr}");
        }
    }
}
