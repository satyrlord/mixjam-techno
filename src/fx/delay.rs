//! The Echoform Delay's signal processing.
//!
//! Each side oversamples its input drive with ADAA2 waveshaping (`2x` polyphase
//! IIR live, `8x` linear-phase FIR offline), reads its delay line with cubic or
//! 16-tap Blackman-sinc interpolation, and circulates through character colouring,
//! modulated diffusion smear, TPT state-variable resonant tone filters, and soft
//! bounding.
use super::adaa::Adaa2State;
use super::oversample::{FirOversampler, HalfBandIir2x, RenderQuality};
use super::parts::{AllPass, Lfo, Line, OnePole, Svf, Tone, bound, widen};
use super::{DelayCharacter, DistortionShape, Echoform};
use crate::master::dsp::{Biquad, Coefficients, DcBlocker, RATE, Smoothed, db_to_gain, pole};
use std::f64::consts::{FRAC_1_SQRT_2, PI};

/// Longest time a line reaches: a dotted whole note at 120 BPM is 3 s + 20 ms mod.
const REACH_SECONDS: f64 = 3.1;
/// Digital time changes crossfade between read heads over 30 ms.
const FADE: usize = 1323;
const WOW_HZ_L: f64 = 0.43;
const WOW_HZ_R: f64 = 0.51;
const FLUTTER_HZ_L: f64 = 6.1;
const FLUTTER_HZ_R: f64 = 6.7;
const NATIVE_WOW_SAMPLES: f64 = 0.000_14 * RATE;
const NATIVE_FLUTTER_SAMPLES: f64 = 0.000_025 * RATE;
const SMEAR_SWING: f64 = 0.000_15 * RATE;

/// The time of one side and the reads at it.
struct Head {
    time: Smoothed,
    from: f64,
    fade: usize,
}
impl Head {
    fn new() -> Self {
        Self {
            time: Smoothed::with_time(RATE * 0.5, 0.08),
            from: 0.0,
            fade: 0,
        }
    }

    #[inline]
    fn sample_line(line: &Line, delay: f64, quality: RenderQuality) -> f64 {
        match quality {
            RenderQuality::Live => line.read(delay),
            RenderQuality::Offline => line.read_sinc(delay),
        }
    }

    #[inline]
    fn sample_taps(line: &Line, delay: f64, compensation: f64, quality: RenderQuality) -> [f64; 2] {
        let feedback = Self::sample_line(line, delay, quality);
        let output = if compensation == 0.0 {
            feedback
        } else {
            Self::sample_line(line, delay - compensation, quality)
        };
        [feedback, output]
    }

    #[inline]
    fn advance(&mut self, digital: bool) {
        if !digital {
            self.time.next();
            return;
        }
        if self.fade == 0 && (self.time.target() - self.time.value()).abs() > 0.5 {
            self.from = self.time.value();
            self.time.settle();
            self.fade = FADE;
        }
    }

    #[inline]
    fn read(
        &mut self,
        line: &Line,
        offset: f64,
        digital: bool,
        compensation: f64,
        quality: RenderQuality,
    ) -> [f64; 2] {
        self.advance(digital);
        let now = Self::sample_taps(line, self.time.value() + offset, compensation, quality);
        if self.fade == 0 {
            return now;
        }
        self.fade -= 1;
        let progress = 1.0 - self.fade as f64 / FADE as f64;
        let weight = 0.5 - 0.5 * (PI * progress).cos();
        let before = Self::sample_taps(line, self.from + offset, compensation, quality);
        std::array::from_fn(|index| before[index] + (now[index] - before[index]) * weight)
    }
}

/// Pre-allocated ADAA2 waveshaper state isolated so oversamplers borrow disjointly.
struct DriveShaper {
    adaa_dig: Adaa2State,
    adaa_ana: Adaa2State,
    adaa_tape: Adaa2State,
    env_fast: f64,
    env: f64,
}
impl DriveShaper {
    fn new() -> Self {
        Self {
            adaa_dig: Adaa2State::default(),
            adaa_ana: Adaa2State::default(),
            adaa_tape: Adaa2State::default(),
            env_fast: 0.0,
            env: 0.0,
        }
    }

    fn clear(&mut self) {
        self.adaa_dig.rest_at(DistortionShape::Overdrive, 0.0);
        self.adaa_ana.rest_at(DistortionShape::Overdrive, 0.0);
        self.adaa_tape.rest_at(DistortionShape::Tape, 0.0);
        self.env_fast = 0.0;
        self.env = 0.0;
    }

    #[inline]
    fn shape(&mut self, s: f64, amount: f64, analog: f64, tape: f64) -> f64 {
        if amount <= 0.0 {
            return s;
        }
        let dig_w = (1.0 - analog - tape).max(0.0);
        let g = 1.0 + 3.0 * amount;
        let dig = if dig_w > 0.0 {
            let shaped = self.adaa_dig.process(DistortionShape::Overdrive, s * g) / g
                * (1.0 + 0.35 * amount);
            s + (shaped - s) * amount
        } else {
            s
        };
        let ana = if analog > 0.0 {
            let ag = 1.0 + 2.6 * amount;
            let bias = 0.14 * (0.25 + 0.75 * amount) * (self.env * ag).min(0.8);
            let shaped = self
                .adaa_ana
                .process(DistortionShape::Overdrive, s * ag + bias)
                / (1.0 + 1.8 * amount);
            s + (shaped - s) * amount
        } else {
            s
        };
        let tp = if tape > 0.0 {
            let tg = 1.0 + 3.2 * amount;
            let bias = 0.38 * (0.3 + 0.7 * amount) * (self.env * tg).min(1.0);
            let shaped =
                self.adaa_tape.process(DistortionShape::Tape, s * tg + bias) / (1.0 + 2.2 * amount);
            s + (shaped - s) * amount
        } else {
            s
        };
        dig_w * dig + analog * ana + tape * tp
    }
}

/// Pre-allocated per-channel input saturation and analog/tape voicing.
struct DriveStage {
    iir: HalfBandIir2x,
    fir: FirOversampler,
    shaper: DriveShaper,
    drive_mix: Smoothed,
    dc: DcBlocker,
    head_bump: Biquad,
    bbd_shelf: OnePole,
    tape_erasure: OnePole,
}
impl DriveStage {
    fn new() -> Self {
        let mut head_bump = Biquad::new(Coefficients::peaking(80.0, 1.4, 1.5));
        head_bump.clear();
        let mut bbd_shelf = OnePole::default();
        bbd_shelf.tune(10_500.0);
        let mut tape_erasure = OnePole::default();
        tape_erasure.tune(18_000.0);
        Self {
            iir: HalfBandIir2x::ZERO,
            fir: FirOversampler::new(false),
            shaper: DriveShaper::new(),
            drive_mix: Smoothed::with_time(0.0, 0.005),
            dc: DcBlocker::ZERO,
            head_bump,
            bbd_shelf,
            tape_erasure,
        }
    }

    fn clear(&mut self) {
        self.iir.clear();
        self.fir.clear();
        self.shaper.clear();
        self.dc = DcBlocker::ZERO;
        self.head_bump.clear();
        self.bbd_shelf.clear();
        self.tape_erasure.clear();
    }

    #[inline]
    fn update_env(&mut self, abs: f64, attack: f64, release: f64, smooth: f64) {
        let coeff = if abs > self.shaper.env_fast {
            attack
        } else {
            release
        };
        self.shaper.env_fast += (abs - self.shaper.env_fast) * coeff;
        self.shaper.env += (self.shaper.env_fast - self.shaper.env) * smooth;
    }

    #[inline]
    fn voice(&mut self, mut y: f64, drive_mix: f64, analog: f64, tape: f64, dc_pole: f64) -> f64 {
        let dc_mix = drive_mix * (analog + tape).min(1.0);
        if dc_mix > 0.0 {
            let filtered = self.dc.process(y, dc_pole);
            y = if dc_mix == 1.0 {
                filtered
            } else {
                y + (filtered - y) * dc_mix
            };
        }
        if analog > 0.0 {
            let bbd = self.bbd_shelf.low(y);
            y += analog * 0.45 * (bbd - y);
        }
        if tape > 0.0 {
            let bump = self.head_bump.process(0, self.tape_erasure.low(y));
            y += tape * (bump - y);
        }
        y
    }

    #[inline]
    fn process(
        &mut self,
        x: f64,
        amount: f64,
        analog: f64,
        tape: f64,
        params: (f64, f64, f64, f64),
        quality: RenderQuality,
    ) -> f64 {
        let (env_attack, env_release, env_smooth, dc_pole) = params;
        self.update_env(x.abs(), env_attack, env_release, env_smooth);

        let drive_mix = self.drive_mix.next();
        let shaper = &mut self.shaper;
        let y = match quality {
            RenderQuality::Offline => self
                .fir
                .process(x, |s| shaper.shape(s, amount, analog, tape)),
            RenderQuality::Live if drive_mix > 0.0 => {
                let driven = self
                    .iir
                    .process(x, |s| shaper.shape(s, amount, analog, tape));
                if drive_mix == 1.0 {
                    driven
                } else {
                    x + (driven - x) * drive_mix
                }
            }
            RenderQuality::Live => x,
        };
        self.voice(y, drive_mix, analog, tape, dc_pole)
    }
}

pub(super) struct Core {
    lines: [Line; 2],
    heads: [Head; 2],
    drives: [DriveStage; 2],
    tone: [Tone; 2],
    svf_hp: [Svf; 2],
    svf_lp: [Svf; 2],
    resonance: Smoothed,
    diffusers: [[AllPass; 2]; 2],
    smear: Smoothed,
    soften: [OnePole; 2],
    high_cut: f64,
    soften_corner: f64,
    dc: [DcBlocker; 2],
    dc_pole: f64,
    env_attack: f64,
    env_release: f64,
    env_smooth: f64,
    lfo: Lfo,
    wow_l: Lfo,
    wow_r: Lfo,
    flutter_l: Lfo,
    flutter_r: Lfo,
    rate: f64,
    feedback: Smoothed,
    cross: Smoothed,
    width: Smoothed,
    output: Smoothed,
    drive: Smoothed,
    last_drive: f64,
    depth: Smoothed,
    analog: Smoothed,
    tape: Smoothed,
    digital: bool,
    compensation: f64,
    activity: f64,
}

impl Core {
    pub(super) const OFFLINE_LATENCY: usize = FirOversampler::LATENCY_8X;

    pub(super) fn new() -> Self {
        let frames = (REACH_SECONDS * RATE) as usize;
        let smooth = |value| Smoothed::with_time(value, 0.005);
        let mut core = Self {
            lines: [Line::new(frames), Line::new(frames)],
            heads: [Head::new(), Head::new()],
            drives: [DriveStage::new(), DriveStage::new()],
            tone: [Tone::default(); 2],
            svf_hp: [Svf::default(); 2],
            svf_lp: [Svf::default(); 2],
            resonance: smooth(0.0),
            diffusers: [[0.0031, 0.0067], [0.0037, 0.0073]].map(|times| {
                times.map(|time| AllPass::with_modulation(time, SMEAR_SWING.ceil() as usize))
            }),
            smear: smooth(0.0),
            soften: [OnePole::default(); 2],
            high_cut: 0.0,
            soften_corner: f64::NAN,
            dc: [DcBlocker::ZERO; 2],
            dc_pole: pole(1.0 / (2.0 * PI * 15.0)),
            env_attack: 1.0 - pole(0.012),
            env_release: 1.0 - pole(0.060),
            env_smooth: 1.0 - pole(0.010),
            lfo: Lfo::default(),
            wow_l: Lfo::default(),
            wow_r: Lfo::default(),
            flutter_l: Lfo::default(),
            flutter_r: Lfo::default(),
            rate: 0.0,
            feedback: smooth(0.0),
            cross: Smoothed::with_time(0.0, 0.01),
            width: smooth(1.0),
            output: smooth(0.0),
            drive: smooth(0.0),
            last_drive: -1.0,
            depth: smooth(0.0),
            analog: Smoothed::with_time(0.0, 0.02),
            tape: Smoothed::with_time(0.0, 0.02),
            digital: true,
            compensation: 0.0,
            activity: 0.0,
        };
        core.wow_r.skip(1.0, (0.37 * RATE) as u64);
        core.flutter_r.skip(1.0, (0.29 * RATE) as u64);
        core
    }

    pub(super) fn set(&mut self, settings: &Echoform, bpm: f64, compensation: f64) {
        self.compensation = compensation;
        self.set_heads(settings, bpm);
        self.set_character(settings);
        self.set_tone(settings);
        self.set_levels(settings);
    }

    fn set_heads(&mut self, settings: &Echoform, bpm: f64) {
        let reach = self.lines[0].reach() - 0.021 * RATE - 2.0;
        let time_s = if settings.character == DelayCharacter::Tape {
            0.35
        } else {
            0.08
        };
        for (head, seconds) in self.heads.iter_mut().zip(settings.seconds(bpm)) {
            head.time.set((seconds * RATE).clamp(2.0, reach));
            head.time.set_time(time_s);
        }
    }

    fn set_character(&mut self, settings: &Echoform) {
        self.digital = settings.character == DelayCharacter::Digital;
        let (analog, tape) = match settings.character {
            DelayCharacter::Digital => (0.0, 0.0),
            DelayCharacter::Analog => (1.0, 0.0),
            DelayCharacter::Tape => (0.0, 1.0),
        };
        self.analog.set(analog);
        self.tape.set(tape);
    }

    fn set_tone(&mut self, settings: &Echoform) {
        let (low_cut, high_cut) = (f64::from(settings.low_cut), f64::from(settings.high_cut));
        for tone in &mut self.tone {
            tone.tune(low_cut, high_cut);
        }
        let resonance = (f64::from(settings.filter_resonance) / 100.0).min(1.0);
        let q = FRAC_1_SQRT_2 + (4.0 - FRAC_1_SQRT_2) * resonance;
        for svf in &mut self.svf_hp {
            svf.tune(low_cut, q);
        }
        for svf in &mut self.svf_lp {
            svf.tune(high_cut, q);
        }
        self.resonance.set(resonance);
        self.smear.set(f64::from(settings.smear_percent) / 100.0);
        self.high_cut = high_cut;
    }

    fn set_levels(&mut self, settings: &Echoform) {
        self.rate = f64::from(settings.mod_rate);
        self.depth
            .set(f64::from(settings.mod_depth) / 1000.0 * RATE);
        self.feedback.set(f64::from(settings.feedback) / 100.0);
        self.cross.set(if settings.ping_pong { 1.0 } else { 0.0 });
        self.width.set(f64::from(settings.width) / 100.0);
        let drive = f64::from(settings.drive) / 100.0;
        self.drive.set(drive);
        for stage in &mut self.drives {
            stage.drive_mix.set(if drive > 0.0 { 1.0 } else { 0.0 });
        }
        if (drive - self.last_drive).abs() > 1e-4 {
            self.last_drive = drive;
            let corner = 18_000.0 - 9_500.0 * drive;
            for stage in &mut self.drives {
                stage.tape_erasure.tune(corner);
            }
        }
        let output = if settings.bypass {
            0.0
        } else {
            db_to_gain(f64::from(settings.output_db))
        };
        self.output.set(output);
    }

    pub(super) fn settle(&mut self) {
        for head in &mut self.heads {
            head.time.settle();
            head.fade = 0;
        }
        for stage in &mut self.drives {
            stage.drive_mix.settle();
        }
        for glide in [
            &mut self.feedback,
            &mut self.cross,
            &mut self.width,
            &mut self.output,
            &mut self.drive,
            &mut self.depth,
            &mut self.resonance,
            &mut self.smear,
            &mut self.analog,
            &mut self.tape,
        ] {
            glide.settle();
        }
    }

    pub(super) fn clear(&mut self) {
        self.lines.iter_mut().for_each(Line::clear);
        self.drives.iter_mut().for_each(DriveStage::clear);
        self.tone.iter_mut().for_each(Tone::clear);
        self.svf_hp.iter_mut().for_each(Svf::clear);
        self.svf_lp.iter_mut().for_each(Svf::clear);
        self.diffusers.iter_mut().flatten().for_each(AllPass::clear);
        self.soften.iter_mut().for_each(OnePole::clear);
        self.dc = [DcBlocker::ZERO; 2];
        self.activity = 0.0;
    }

    pub(super) fn skip(&mut self, frames: u64) {
        self.lfo.skip(self.rate, frames);
        self.wow_l.skip(WOW_HZ_L, frames);
        self.wow_r.skip(WOW_HZ_R, frames);
        self.flutter_l.skip(FLUTTER_HZ_L, frames);
        self.flutter_r.skip(FLUTTER_HZ_R, frames);
    }

    pub(super) fn activity(&self) -> f64 {
        self.activity
    }

    pub(super) fn silent_gap(&self) -> usize {
        self.heads
            .iter()
            .map(|head| head.time.value().max(head.time.target()).max(head.from))
            .fold(0.0, f64::max) as usize
            + (0.05 * RATE) as usize
    }

    #[inline]
    fn tune_soften(&mut self, analog: f64, tape: f64) {
        let corner = self.high_cut * (1.0 - 0.25 * analog - 0.45 * tape);
        if corner != self.soften_corner {
            self.soften_corner = corner;
            for soften in &mut self.soften {
                soften.tune(corner);
            }
        }
    }

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
    fn modulation_offsets(&mut self, tape: f64, depth: f64) -> [f64; 2] {
        let wander_l = tape
            * ((NATIVE_WOW_SAMPLES + 0.35 * depth) * self.wow_l.value(0.0)
                + (NATIVE_FLUTTER_SAMPLES + 0.05 * depth) * self.flutter_l.value(0.0));
        let wander_r = tape
            * ((NATIVE_WOW_SAMPLES + 0.35 * depth) * self.wow_r.value(0.0)
                + (NATIVE_FLUTTER_SAMPLES + 0.05 * depth) * self.flutter_r.value(0.0));
        let offsets = [
            depth * self.lfo.value(0.0) + wander_l,
            depth * self.lfo.value(0.25) + wander_r,
        ];
        self.lfo.advance(self.rate);
        self.wow_l.advance(WOW_HZ_L);
        self.wow_r.advance(WOW_HZ_R);
        self.flutter_l.advance(FLUTTER_HZ_L);
        self.flutter_r.advance(FLUTTER_HZ_R);
        offsets
    }

    #[inline]
    fn smear_feedback(&mut self, mut back: [f64; 2], smear: f64) -> [f64; 2] {
        if smear <= 0.0 {
            return back;
        }
        let drift = SMEAR_SWING * self.wow_l.value(0.0);
        for (side, sample) in back.iter_mut().enumerate() {
            let d = if side == 0 { drift } else { -drift };
            let [first, second] = &mut self.diffusers[side];
            let spread = first.process_modulated(*sample, 0.65, d);
            let spread = second.process_modulated(spread, 0.65, -d);
            *sample += (spread - *sample) * smear;
        }
        back
    }

    #[inline]
    fn push_feedback(&mut self, input: [f64; 2], back: [f64; 2], gain: f64, resonance: f64) {
        for side in 0..2 {
            let feed = input[side] + gain * back[side];
            let mut sample = self.tone[side].process(feed);
            if resonance > 0.0 {
                let hp = self.svf_hp[side].high(feed);
                let resonant = self.svf_lp[side].low(hp);
                sample += (resonant - sample) * resonance;
            }
            self.lines[side].push(bound(sample) as f32);
        }
    }

    #[inline]
    pub(super) fn process_with_quality(
        &mut self,
        left: f64,
        right: f64,
        quality: RenderQuality,
    ) -> (f64, f64) {
        let amount = self.drive.next();
        let (analog, tape) = (self.analog.next(), self.tape.next());
        let env_params = (
            self.env_attack,
            self.env_release,
            self.env_smooth,
            self.dc_pole,
        );
        let left = self.drives[0].process(left, amount, analog, tape, env_params, quality);
        let right = self.drives[1].process(right, amount, analog, tape, env_params, quality);

        self.tune_soften(analog, tape);
        let depth = self.depth.next();
        let offsets = self.modulation_offsets(tape, depth);
        let taps = [
            self.heads[0].read(
                &self.lines[0],
                offsets[0],
                self.digital,
                self.compensation,
                quality,
            ),
            self.heads[1].read(
                &self.lines[1],
                offsets[1],
                self.digital,
                self.compensation,
                quality,
            ),
        ];
        self.activity = taps.iter().flatten().fold(0.0_f64, |p, s| p.max(s.abs()));
        let coloured = [
            self.colour(0, taps[0][0], analog, tape),
            self.colour(1, taps[1][0], analog, tape),
        ];
        let cross = self.cross.next();
        let back = [
            coloured[0] + (coloured[1] - coloured[0]) * cross,
            coloured[1] + (coloured[0] - coloured[1]) * cross,
        ];
        let smear = self.smear.next();
        let back = self.smear_feedback(back, smear);
        let input = [
            left + (0.5 * (left + right) - left) * cross,
            right * (1.0 - cross),
        ];
        let gain = self.feedback.next();
        let resonance = self.resonance.next();
        self.push_feedback(input, back, gain, resonance);
        let (l, r) = widen(taps[0][1], taps[1][1], self.width.next());
        let level = self.output.next();
        (l * level, r * level)
    }

    #[cfg(test)]
    #[inline]
    pub(super) fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        self.process_with_quality(left, right, RenderQuality::Live)
    }
}

fn sample_ping_pong_taps(
    t_l: f64,
    t_r: f64,
    horizon: f64,
    fb: f64,
    width: f64,
    out: &mut [(f32, f32, f32)],
) -> usize {
    let (mut t, mut on_left, mut amp, mut written) = (t_l, true, 1.0, 0);
    while written < out.len() && t <= horizon && amp >= 0.01 {
        let (raw_l, raw_r) = if on_left { (amp, 0.0) } else { (0.0, amp) };
        let (wl, wr) = widen(raw_l, raw_r, width);
        out[written] = ((t / horizon) as f32, wl as f32, wr as f32);
        written += 1;
        t += if on_left { t_r } else { t_l };
        on_left = !on_left;
        amp *= fb;
    }
    written
}

fn sample_dual_taps(
    times: [f64; 2],
    horizon: f64,
    feedback: f64,
    width: f64,
    out: &mut [(f32, f32, f32)],
) -> usize {
    let mut next = times;
    let mut repeat = [1; 2];
    let mut amplitude = [1.0; 2];
    let mut written = 0;
    for slot in out {
        let time = next[0].min(next[1]);
        if time > horizon {
            break;
        }
        let raw: [f64; 2] = std::array::from_fn(|side| {
            if (next[side] - time).abs() > 1e-12 {
                return 0.0;
            }
            let tap = amplitude[side];
            amplitude[side] *= feedback;
            repeat[side] += 1;
            next[side] = if amplitude[side] < 0.01 {
                f64::INFINITY
            } else {
                times[side] * f64::from(repeat[side])
            };
            tap
        });
        let (left, right) = widen(raw[0], raw[1], width);
        *slot = ((time / horizon) as f32, left as f32, right as f32);
        written += 1;
    }
    written
}

/// Sample normalized `(norm_time, left_amp, right_amp)` echo impulses into `out`
/// for the `Tap & Decay` Deep Edit scope visualizer.
pub(crate) fn sample_delay_taps(
    settings: &Echoform,
    bpm: f64,
    out: &mut [(f32, f32, f32)],
) -> usize {
    let [t_l, t_r] = settings.seconds(bpm).map(|s| s.max(0.001));
    let horizon = (4.0 * t_l.max(t_r)).max(0.5);
    let fb = f64::from(settings.feedback) / 100.0;
    let width = f64::from(settings.width) / 100.0;
    if settings.ping_pong {
        sample_ping_pong_taps(t_l, t_r, horizon, fb, width, out)
    } else {
        sample_dual_taps([t_l, t_r], horizon, fb, width, out)
    }
}

#[cfg(test)]
#[path = "delay/tests.rs"]
mod tests;
