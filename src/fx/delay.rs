//! The Echoform Delay's signal processing.
//!
//! Each side reads its delay line at its time plus modulation. The repeats
//! pass the character's colouring and, in ping-pong, cross to the other
//! side; scaled by the feedback, they join the new input, and the low-cut
//! and high-cut filters shape the sum before it enters the line. So each
//! repeat is filtered once more than the one before. The output is the
//! read taps only, widened, ducked, and trimmed.
use super::parts::{Ducker, Glide, Lfo, Line, OnePole, Tone, bound, drive, widen};
use super::{DelayCharacter, Echoform};
use crate::master::dsp::{DcBlocker, RATE, db_to_gain, pole};
use std::f64::consts::PI;

/// The longest time a line reaches: a dotted whole note at 120 BPM lasts
/// 3 s, and the modulation adds up to 20 ms on top.
const REACH_SECONDS: f64 = 3.1;
/// A digital time change crossfades between two read heads over 30 ms.
const FADE: usize = 1323;

/// The time of one side and the reads at it.
struct Head {
    time: Glide,
    /// The time the digital crossfade comes from, while it runs.
    from: f64,
    fade: usize,
}
impl Head {
    fn new() -> Self {
        Self {
            time: Glide::new(RATE * 0.5, 0.08),
            from: 0.0,
            fade: 0,
        }
    }
    /// The line at the current time plus `offset` frames. A digital head
    /// jumps to a new time through a crossfade; the others glide to it.
    #[inline]
    fn read(&mut self, line: &Line, offset: f64, digital: bool, compensation: f64) -> [f64; 2] {
        if digital {
            if self.fade == 0 && (self.time.target() - self.time.value()).abs() > 0.5 {
                self.from = self.time.value();
                self.time.settle();
                self.fade = FADE;
            }
        } else {
            self.time.next();
        }
        let now =
            [0.0, compensation].map(|advance| line.read(self.time.value() + offset - advance));
        if self.fade == 0 {
            return now;
        }
        self.fade -= 1;
        let progress = 1.0 - self.fade as f64 / FADE as f64;
        let weight = 0.5 - 0.5 * (PI * progress).cos();
        std::array::from_fn(|index| {
            let advance = if index == 0 { 0.0 } else { compensation };
            let before = line.read(self.from + offset - advance);
            before + (now[index] - before) * weight
        })
    }
}

pub(super) struct Core {
    lines: [Line; 2],
    heads: [Head; 2],
    /// The loop's low-cut and high-cut.
    tone: [Tone; 2],
    /// The extra high-frequency loss of the analog and tape characters.
    soften: [OnePole; 2],
    dc: [DcBlocker; 2],
    dc_pole: f64,
    lfo: Lfo,
    wow: Lfo,
    flutter: Lfo,
    ducker: Ducker,
    rate: f64,
    feedback: Glide,
    cross: Glide,
    width: Glide,
    output: Glide,
    drive: Glide,
    depth: Glide,
    duck: Glide,
    analog: Glide,
    tape: Glide,
    digital: bool,
    compensation: f64,
    activity: f64,
}
impl Core {
    pub(super) fn new() -> Self {
        let frames = (REACH_SECONDS * RATE) as usize;
        let smooth = |value| Glide::new(value, 0.005);
        Self {
            lines: [Line::new(frames), Line::new(frames)],
            heads: [Head::new(), Head::new()],
            tone: [Tone::default(); 2],
            soften: [OnePole::default(); 2],
            dc: [DcBlocker::ZERO; 2],
            dc_pole: pole(1.0 / (2.0 * PI * 5.0)),
            lfo: Lfo::default(),
            wow: Lfo::default(),
            flutter: Lfo::default(),
            ducker: Ducker::new(),
            rate: 0.0,
            feedback: smooth(0.0),
            cross: Glide::new(0.0, 0.01),
            width: smooth(1.0),
            output: smooth(0.0),
            drive: smooth(0.0),
            depth: smooth(0.0),
            duck: smooth(0.0),
            analog: Glide::new(0.0, 0.02),
            tape: Glide::new(0.0, 0.02),
            digital: true,
            compensation: 0.0,
            activity: 0.0,
        }
    }
    /// Take new settings at a tempo. Advance only the output taps for the
    /// safety limiter; feedback retains the full delay time.
    pub(super) fn set(&mut self, settings: &Echoform, bpm: f64, compensation: f64) {
        self.compensation = compensation;
        let reach = self.lines[0].reach() - 0.021 * RATE - 2.0;
        for (head, seconds) in self.heads.iter_mut().zip(settings.seconds(bpm)) {
            head.time.set((seconds * RATE).clamp(2.0, reach));
            head.time.set_time(match settings.character {
                DelayCharacter::Tape => 0.35,
                _ => 0.08,
            });
        }
        self.digital = settings.character == DelayCharacter::Digital;
        let (analog, tape) = match settings.character {
            DelayCharacter::Digital => (0.0, 0.0),
            DelayCharacter::Analog => (1.0, 0.0),
            DelayCharacter::Tape => (0.0, 1.0),
        };
        self.analog.set(analog);
        self.tape.set(tape);
        let (low_cut, high_cut) = (f64::from(settings.low_cut), f64::from(settings.high_cut));
        for (tone, soften) in self.tone.iter_mut().zip(&mut self.soften) {
            tone.tune(low_cut, high_cut);
            soften.tune(high_cut * (1.0 - 0.25 * analog - 0.45 * tape));
        }
        self.rate = f64::from(settings.mod_rate);
        self.depth
            .set(f64::from(settings.mod_depth) / 1000.0 * RATE);
        self.feedback.set(f64::from(settings.feedback) / 100.0);
        self.cross.set(if settings.ping_pong { 1.0 } else { 0.0 });
        self.width.set(f64::from(settings.width) / 100.0);
        self.drive.set(f64::from(settings.drive) / 100.0);
        self.duck.set(f64::from(settings.duck_amount) / 100.0);
        self.ducker.set_release(f64::from(settings.duck_release));
        let output = if settings.bypass {
            0.0
        } else {
            db_to_gain(f64::from(settings.output_db))
        };
        self.output.set(output);
    }
    /// Move every setting straight to its target.
    pub(super) fn settle(&mut self) {
        for head in &mut self.heads {
            head.time.settle();
            head.fade = 0;
        }
        for glide in [
            &mut self.feedback,
            &mut self.cross,
            &mut self.width,
            &mut self.output,
            &mut self.drive,
            &mut self.depth,
            &mut self.duck,
            &mut self.analog,
            &mut self.tape,
        ] {
            glide.settle();
        }
    }
    /// Silence the lines and every filter.
    pub(super) fn clear(&mut self) {
        self.lines.iter_mut().for_each(Line::clear);
        self.tone.iter_mut().for_each(Tone::clear);
        self.soften.iter_mut().for_each(OnePole::clear);
        self.dc = [DcBlocker::ZERO; 2];
        self.ducker.clear();
        self.activity = 0.0;
    }
    /// Activity before output mute or ducking, including the feedback taps.
    pub(super) fn activity(&self) -> f64 {
        self.activity
    }

    /// Let a pending repeat reach the read head before deciding the line is silent.
    pub(super) fn silent_gap(&self) -> usize {
        self.heads
            .iter()
            .map(|head| head.time.value().max(head.time.target()).max(head.from))
            .fold(0.0, f64::max) as usize
            + (0.05 * RATE) as usize
    }
    /// The colouring of the analog and tape characters, blended by their weights.
    #[inline]
    fn colour(&mut self, side: usize, x: f64, analog: f64, tape: f64) -> f64 {
        if analog == 0.0 && tape == 0.0 {
            return x;
        }
        let warm = (1.2 * x).tanh() / 1.2;
        let bias = 0.12;
        let worn = self.dc[side].process(
            ((1.6 * (x + bias)).tanh() - (1.6 * bias).tanh()) / 1.6,
            self.dc_pole,
        );
        let shaped = x + analog * (warm - x) + tape * (worn - x);
        let soft = self.soften[side].low(shaped);
        shaped + (analog + tape).min(1.0) * (soft - shaped)
    }
    #[inline]
    pub(super) fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        let duck = self.ducker.gain(left, right, self.duck.next());
        let amount = self.drive.next();
        let (left, right) = (drive(left, amount), drive(right, amount));
        let (analog, tape) = (self.analog.next(), self.tape.next());
        let depth = self.depth.next();
        let wander = tape * depth * (0.6 * self.wow.value(0.0) + 0.08 * self.flutter.value(0.0));
        let offsets = [
            depth * self.lfo.value(0.0) + wander,
            depth * self.lfo.value(0.25) + wander,
        ];
        self.lfo.advance(self.rate);
        self.wow.advance(0.53);
        self.flutter.advance(6.1);
        let digital = self.digital;
        let taps = [
            self.heads[0].read(&self.lines[0], offsets[0], digital, self.compensation),
            self.heads[1].read(&self.lines[1], offsets[1], digital, self.compensation),
        ];
        self.activity = taps
            .iter()
            .flatten()
            .fold(0.0_f64, |peak, sample| peak.max(sample.abs()));
        let coloured = [
            self.colour(0, taps[0][0], analog, tape),
            self.colour(1, taps[1][0], analog, tape),
        ];
        // Ping-pong crosses the repeats and feeds the input to the left only.
        let cross = self.cross.next();
        let back = [
            coloured[0] + (coloured[1] - coloured[0]) * cross,
            coloured[1] + (coloured[0] - coloured[1]) * cross,
        ];
        let input = [
            left + (0.5 * (left + right) - left) * cross,
            right * (1.0 - cross),
        ];
        let gain = self.feedback.next();
        for side in 0..2 {
            let sample = bound(self.tone[side].process(input[side] + gain * back[side]));
            self.lines[side].push(sample as f32);
        }
        let (l, r) = widen(taps[0][1], taps[1][1], self.width.next());
        let level = self.output.next() * duck;
        (l * level, r * level)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn delay(settings: &Echoform, bpm: f64) -> Core {
        let mut core = Core::new();
        core.set(settings, bpm, 0.0);
        core.settle();
        core
    }

    /// The frames after an impulse at frame 0 where a side passes `level`.
    fn arrivals(core: &mut Core, frames: usize, side: usize) -> Vec<usize> {
        let mut out = Vec::new();
        let mut above = false;
        for i in 0..frames {
            let x = if i == 0 { 1.0 } else { 0.0 };
            let (l, r) = core.process(x, x);
            let value = [l, r][side].abs();
            if value > 0.05 && !above {
                out.push(i);
            }
            above = value > 0.05;
        }
        out
    }

    fn plain() -> Echoform {
        Echoform {
            character: DelayCharacter::Digital,
            ping_pong: false,
            width: 100.0,
            low_cut: 20.0,
            high_cut: 20_000.0,
            mod_depth: 0.0,
            duck_amount: 0.0,
            output_db: 0.0,
            feedback: 50.0,
            ..Echoform::default()
        }
    }

    #[test]
    fn synced_repeats_land_on_their_note_lengths_at_each_tempo() {
        for bpm in [120.0, 125.0, 140.0, 180.0] {
            let settings = Echoform {
                division_l: super::super::Division::Quarter,
                division_r: super::super::Division::EighthDotted,
                ..plain()
            };
            let mut core = delay(&settings, bpm);
            let left = arrivals(&mut core, 200_000, 0);
            let quarter = 60.0 / bpm * RATE;
            for (k, &frame) in left.iter().take(3).enumerate() {
                let expected = quarter * (k + 1) as f64;
                assert!(
                    (frame as f64 - expected).abs() <= 2.0,
                    "{bpm}: {frame} vs {expected}"
                );
            }
            let mut core = delay(&settings, bpm);
            let right = arrivals(&mut core, 100_000, 1);
            assert!((right[0] as f64 - 0.75 * quarter).abs() <= 2.0);
        }
    }

    #[test]
    fn free_times_and_the_compensation_set_each_side() {
        let settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 100.0,
            time_ms_r: 250.0,
            ..plain()
        };
        let mut core = Core::new();
        core.set(&settings, 120.0, 220.0);
        core.settle();
        assert_eq!(arrivals(&mut core, 20_000, 0)[0], 4410 - 220);
        let mut core = Core::new();
        core.set(&settings, 120.0, 220.0);
        core.settle();
        assert_eq!(arrivals(&mut core, 20_000, 1)[0], 11_025 - 220);
    }

    #[test]
    fn ping_pong_alternates_sides_and_stereo_keeps_them() {
        let settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 100.0,
            time_ms_r: 100.0,
            ping_pong: true,
            ..plain()
        };
        let mut core = delay(&settings, 120.0);
        let mut energy = [[0.0; 2]; 4];
        for i in 0..4 * 4410 + 2000 {
            let (l, r) = core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0);
            let slot = i / 4410;
            if (1..=4).contains(&slot) && i % 4410 < 50 {
                energy[slot - 1][0] += l * l;
                energy[slot - 1][1] += r * r;
            }
        }
        // First repeat left, second right, third left.
        assert!(energy[0][0] > 100.0 * energy[0][1]);
        assert!(energy[1][1] > 100.0 * energy[1][0]);
        assert!(energy[2][0] > 100.0 * energy[2][1]);
    }

    #[test]
    fn feedback_decays_below_unity_and_stays_bounded_above_it() {
        let peak_after = |feedback: f32, character| {
            let settings = Echoform {
                mode: super::super::TimeMode::Free,
                time_ms_l: 50.0,
                time_ms_r: 50.0,
                feedback,
                character,
                ..plain()
            };
            let mut core = delay(&settings, 120.0);
            let mut late: f64 = 0.0;
            for i in 0..20 * 44_100 {
                let x = if i < 2205 {
                    0.8 * (i as f64 * 0.07).sin()
                } else {
                    0.0
                };
                let (l, r) = core.process(x, x);
                assert!(l.is_finite() && r.is_finite());
                if i > 18 * 44_100 {
                    late = late.max(l.abs()).max(r.abs());
                }
            }
            late
        };
        assert!(peak_after(60.0, DelayCharacter::Digital) < 1e-6);
        for character in [
            DelayCharacter::Digital,
            DelayCharacter::Analog,
            DelayCharacter::Tape,
        ] {
            let held = peak_after(110.0, character);
            assert!(held < 1.6, "{character:?}: {held}");
        }
    }

    #[test]
    fn the_loop_filters_darken_each_repeat() {
        let settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 100.0,
            time_ms_r: 100.0,
            feedback: 90.0,
            high_cut: 2000.0,
            ..plain()
        };
        let mut core = delay(&settings, 120.0);
        // The 6 kHz content of a burst falls in each repeat by much more than
        // the feedback alone: the high cut takes some of it every time.
        let mut amplitude = [(0.0, 0.0); 6];
        for i in 0..7 * 4410 {
            let x = if i < 441 {
                (2.0 * PI * 6000.0 * i as f64 / RATE).sin()
            } else {
                0.0
            };
            let (l, _) = core.process(x, x);
            let slot = i / 4410;
            if (1..=6).contains(&slot) {
                let angle = 2.0 * PI * 6000.0 * i as f64 / RATE;
                amplitude[slot - 1].0 += l * angle.cos();
                amplitude[slot - 1].1 += l * angle.sin();
            }
        }
        let levels: Vec<f64> = amplitude.iter().map(|(re, im)| re.hypot(*im)).collect();
        for pair in levels.windows(2) {
            assert!(pair[1] < pair[0] * 0.9 * 0.5, "{levels:?}");
        }
    }

    #[test]
    fn width_and_bypass_shape_only_the_output() {
        let settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 30.0,
            time_ms_r: 60.0,
            width: 0.0,
            ..plain()
        };
        let mut core = delay(&settings, 120.0);
        for i in 0..10_000 {
            let (l, r) = core.process(if i == 0 { 1.0 } else { 0.0 }, 0.0);
            assert!((l - r).abs() < 1e-12);
        }
        // Bypass silences the output while the loop keeps the repeats.
        let mut core = delay(&plain(), 120.0);
        core.process(1.0, 1.0);
        let mut bypassed = plain();
        bypassed.bypass = true;
        core.set(&bypassed, 120.0, 0.0);
        core.settle();
        for _ in 0..30_000 {
            assert_eq!(core.process(0.0, 0.0), (0.0, 0.0));
        }
        core.set(&plain(), 120.0, 0.0);
        core.settle();
        let mut heard: f64 = 0.0;
        for _ in 0..60_000 {
            heard = heard.max(core.process(0.0, 0.0).0.abs());
        }
        assert!(heard > 0.01, "the tail rang on behind the bypass");
    }

    #[test]
    fn modulation_stays_within_its_depth_and_zero_depth_holds_still() {
        let settings = Echoform {
            mode: super::super::TimeMode::Free,
            time_ms_l: 200.0,
            time_ms_r: 200.0,
            feedback: 0.0,
            mod_depth: 0.0,
            character: DelayCharacter::Tape,
            ..plain()
        };
        let mut core = delay(&settings, 120.0);
        // A steady tone through a still delay keeps its level exactly.
        let mut levels = Vec::new();
        for i in 0..60_000 {
            let x = (2.0 * PI * 440.0 * i as f64 / RATE).sin() * 0.5;
            let (l, _) = core.process(x, x);
            if i > 20_000 {
                levels.push(l);
            }
        }
        let peak = levels.iter().fold(0.0_f64, |a, v| a.max(v.abs()));
        assert!(peak < 0.5 + 1e-3, "{peak}");
    }
}
