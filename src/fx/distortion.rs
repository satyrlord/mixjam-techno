//! The Emberform Drive's signal processing: Live 2x polyphase IIR and
//! Offline 8x/16x linear-phase FIR oversampling + ADAA2, envelope-coupled
//! dynamic bias, pre-filtered clock-jittered Bitcrush, and gated level match.
use super::adaa::{Adaa2State, biased_input, crusher_levels, pre_gain, quantize};
use super::oversample::{FirOversampler, HalfBandIir2x, RenderQuality};
use super::parts::{Line, OnePole, widen};
use super::{DistortionShape, Emberform};
use crate::master::dsp::{
    Biquad, Coefficients, DcBlocker, POWER_FLOOR, PowerDetector, RATE, Smoothed, cascade,
    db_to_gain, pole,
};
use std::f64::consts::PI;

/// A shape change crossfades between the old and new curves over 10 ms.
const FADE: usize = 441;
/// Level match detector smoothing time, silence freeze gate, and slew limit.
const LEVEL_TIME: f64 = 0.1;
/// The level match reaches 18 dB: `10^(18 / 20)`.
const MATCH_LIMIT: f64 = 7.943_282_347_242_815;
const GATE_RMS_SQ: f64 = 1.584_893_192_461_114e-6; // -58 dBFS RMS
const MAX_GAIN_SLEW_PER_SAMPLE: f64 = 0.000_063; // ~24 dB/s @ 44.1 kHz
const PRE_AA_QS: [f64; 4] = [0.509_795_58, 0.601_344_89, 0.899_976_22, 2.562_915_45];
const POST_AA_QS: [f64; 2] = [0.541_196_10, 1.306_562_96];

#[derive(Clone, Copy)]
struct Side {
    low_cut: [OnePole; 2],
    high_cut: [OnePole; 2],
    soften: OnePole,
    dc: DcBlocker,
    bias_env: f64,
    adaa_now: Adaa2State,
    adaa_prev: Adaa2State,
    iir: HalfBandIir2x,
    fir_8x: FirOversampler,
    fir_16x: FirOversampler,
    held: f64,
}
impl Default for Side {
    fn default() -> Self {
        Self {
            low_cut: [OnePole::default(); 2],
            high_cut: [OnePole::default(); 2],
            soften: OnePole::default(),
            dc: DcBlocker::ZERO,
            bias_env: 0.0,
            adaa_now: Adaa2State::default(),
            adaa_prev: Adaa2State::default(),
            iir: HalfBandIir2x::ZERO,
            fir_8x: FirOversampler::new(false),
            fir_16x: FirOversampler::new(true),
            held: 0.0,
        }
    }
}

impl Side {
    fn analog(
        &mut self,
        driven: f64,
        shape: DistortionShape,
        from: Option<DistortionShape>,
        weight: f64,
        quality: RenderQuality,
    ) -> (f64, usize) {
        let mut curve = |input| match from {
            Some(old) if shape == DistortionShape::Bitcrush => self.adaa_prev.process(old, input),
            Some(old) if old != DistortionShape::Bitcrush => {
                let prev = self.adaa_prev.process(old, input);
                let now = self.adaa_now.process(shape, input);
                prev + (now - prev) * weight
            }
            _ => self.adaa_now.process(shape, input),
        };
        let main = if shape == DistortionShape::Bitcrush {
            from.expect("a fading analog shape")
        } else {
            shape
        };
        match quality {
            RenderQuality::Live => (self.iir.process(driven, &mut curve), 0),
            RenderQuality::Offline
                if matches!(main, DistortionShape::Foldback | DistortionShape::Rectifier) =>
            {
                (
                    self.fir_16x.process(driven, &mut curve),
                    FirOversampler::LATENCY_16X,
                )
            }
            RenderQuality::Offline => (
                self.fir_8x.process(driven, &mut curve),
                FirOversampler::LATENCY_8X,
            ),
        }
    }
}

pub(super) struct Core {
    sides: [Side; 2],
    crush_pre: [Biquad; 4],
    crush_post: [Biquad; 2],
    shape: DistortionShape,
    from: DistortionShape,
    fade: usize,
    phase: f64,
    step: f64,
    levels: f64,
    jitter_seed: u64,
    crush_rate_hz: f64,
    high_cut_hz: f64,
    level_match: bool,
    matched_gain: f64,
    tape: Smoothed,
    gain: Smoothed,
    asym: Smoothed,
    width: Smoothed,
    output: Smoothed,
    dc_pole: f64,
    env_attack: f64,
    env_release: f64,
    level_pole: f64,
    detectors: [PowerDetector; 2],
    pad_lines: [Line; 2],
    clean_delay: [Line; 2],
    activity: f64,
}

impl Core {
    pub(super) const OFFLINE_LATENCY: usize = FirOversampler::LATENCY_16X;

    pub(super) fn new() -> Self {
        let smooth = |value| Smoothed::with_time(value, 0.005);
        Self {
            sides: [Side::default(); 2],
            crush_pre: [Biquad::FLAT; 4],
            crush_post: [Biquad::FLAT; 2],
            shape: DistortionShape::Tube,
            from: DistortionShape::Tube,
            fade: 0,
            phase: 0.0,
            step: 1.0,
            levels: 128.0,
            jitter_seed: 0x9E37_79B9_7F4A_7C15,
            crush_rate_hz: 11_025.0,
            high_cut_hz: 12_000.0,
            level_match: true,
            matched_gain: 1.0,
            tape: Smoothed::with_time(0.0, 0.01),
            gain: smooth(1.0),
            asym: smooth(0.0),
            width: smooth(1.0),
            output: smooth(0.0),
            dc_pole: pole(1.0 / (2.0 * PI * 15.0)),
            env_attack: 1.0 - pole(0.002),
            env_release: 1.0 - pole(0.025),
            level_pole: pole(LEVEL_TIME),
            detectors: [PowerDetector::default(); 2],
            pad_lines: [
                Line::new(Self::OFFLINE_LATENCY + 4),
                Line::new(Self::OFFLINE_LATENCY + 4),
            ],
            clean_delay: [
                Line::new(Self::OFFLINE_LATENCY + 4),
                Line::new(Self::OFFLINE_LATENCY + 4),
            ],
            activity: 0.0,
        }
    }

    /// Take new settings. A new shape fades in from the old one.
    pub(super) fn set(&mut self, settings: &Emberform) {
        if settings.shape != self.shape {
            self.from = self.shape;
            self.shape = settings.shape;
            self.fade = FADE;
            for side in &mut self.sides {
                side.adaa_prev = side.adaa_now;
                side.adaa_now.retarget(self.shape);
            }
        }
        let low_cut = f64::from(settings.low_cut_hz);
        let high_cut = f64::from(settings.high_cut_hz);
        self.high_cut_hz = high_cut;
        self.crush_rate_hz = f64::from(settings.crush_rate_hz);
        self.level_match = settings.level_match;
        let aa_cutoff = (0.30 * self.crush_rate_hz).clamp(200.0, 19_500.0);
        let post_cutoff = high_cut
            .min(0.45 * self.crush_rate_hz)
            .clamp(220.0, 19_500.0);
        for (stage, &q) in self.crush_pre.iter_mut().zip(&PRE_AA_QS) {
            stage.set(Coefficients::low_pass(aa_cutoff, q));
        }
        for (stage, &q) in self.crush_post.iter_mut().zip(&POST_AA_QS) {
            stage.set(Coefficients::low_pass(post_cutoff, q));
        }
        for side in &mut self.sides {
            side.low_cut.iter_mut().for_each(|p| p.tune(low_cut));
            side.high_cut.iter_mut().for_each(|p| p.tune(high_cut));
            side.soften.tune(0.45 * high_cut);
        }
        self.step = self.crush_rate_hz / RATE;
        self.levels = crusher_levels(settings.bits);
        self.tape.set(if settings.shape == DistortionShape::Tape {
            1.0
        } else {
            0.0
        });
        self.gain
            .set(db_to_gain(f64::from(settings.drive_db)) * pre_gain(settings.shape));
        self.asym.set(f64::from(settings.asymmetry_percent) / 100.0);
        self.width.set(f64::from(settings.width_percent) / 100.0);
        self.output.set(if settings.bypass {
            0.0
        } else {
            db_to_gain(f64::from(settings.output_db))
        });
    }

    pub(super) fn settle(&mut self) {
        for glide in [
            &mut self.tape,
            &mut self.gain,
            &mut self.asym,
            &mut self.width,
            &mut self.output,
        ] {
            glide.settle();
        }
        self.rest();
    }

    pub(super) fn clear(&mut self) {
        for side in &mut self.sides {
            side.low_cut.iter_mut().for_each(OnePole::clear);
            side.high_cut.iter_mut().for_each(OnePole::clear);
            side.soften.clear();
            side.iir.clear();
            side.fir_8x.clear();
            side.fir_16x.clear();
            side.bias_env = 0.0;
        }
        self.crush_pre.iter_mut().for_each(Biquad::clear);
        self.crush_post.iter_mut().for_each(Biquad::clear);
        self.pad_lines.iter_mut().for_each(Line::clear);
        self.clean_delay.iter_mut().for_each(Line::clear);
        self.phase = 0.0;
        self.matched_gain = 1.0;
        self.detectors = [PowerDetector::default(); 2];
        self.activity = 0.0;
        self.rest();
    }

    fn rest(&mut self) {
        self.fade = 0;
        self.from = self.shape;
        for side in &mut self.sides {
            side.adaa_now.rest_at(self.shape, 0.0);
            side.adaa_prev.rest_at(self.from, 0.0);
            side.dc = DcBlocker::ZERO;
            side.held = 0.0;
            side.bias_env = 0.0;
        }
    }

    pub(super) fn activity(&self) -> f64 {
        self.activity
    }

    #[inline]
    fn next_rand(&mut self) -> f64 {
        self.jitter_seed ^= self.jitter_seed << 13;
        self.jitter_seed ^= self.jitter_seed >> 7;
        self.jitter_seed ^= self.jitter_seed << 17;
        ((self.jitter_seed >> 40) as f64 / (1u64 << 24) as f64) * 2.0 - 1.0
    }

    #[inline]
    fn align_offline(&mut self, side: usize, sample: f64, used_latency: usize) -> f64 {
        let extra = Self::OFFLINE_LATENCY - used_latency;
        if extra == 0 {
            return sample;
        }
        self.pad_lines[side].push(sample as f32);
        f64::from(self.pad_lines[side].at(extra + 1))
    }

    #[inline]
    #[cfg(test)]
    pub(super) fn process(&mut self, left: f64, right: f64) -> (f64, f64) {
        self.process_with_quality(left, right, RenderQuality::Offline)
    }

    fn crush_step(&mut self, active_input: bool, asym: f64, crushing_any: bool) -> (bool, f64) {
        let jitter = if asym > 0.0 && active_input {
            0.03 * asym * self.next_rand()
        } else {
            0.0
        };
        let dither = if crushing_any && active_input {
            0.5 * (self.next_rand() + self.next_rand()) / self.levels
        } else {
            0.0
        };
        self.phase += self.step * (1.0 + jitter).max(0.25);
        let hold = self.phase >= 1.0;
        if hold {
            self.phase -= self.phase.floor();
        }
        (hold, dither)
    }

    fn next_fade_weight(&mut self) -> f64 {
        if self.fade == 0 {
            return 1.0;
        }
        self.fade -= 1;
        let progress = 1.0 - self.fade as f64 / FADE as f64;
        0.5 - 0.5 * (PI * progress).cos()
    }

    #[expect(clippy::too_many_arguments)]
    fn shape_crushed(
        &mut self,
        index: usize,
        driven: f64,
        from: Option<DistortionShape>,
        weight: f64,
        hold: bool,
        dither: f64,
        quality: RenderQuality,
    ) -> (f64, usize) {
        let crush_filter = self.crush_rate_hz < 20_000.0;
        let pre = if crush_filter {
            cascade(&mut self.crush_pre, index, driven)
        } else {
            driven
        };
        let side = &mut self.sides[index];
        if hold {
            let noise = if pre.abs() > 1e-6 { dither } else { 0.0 };
            side.held = quantize(pre, self.levels, noise);
        }
        let rec = if crush_filter && self.high_cut_hz < 19_500.0 {
            cascade(&mut self.crush_post, index, side.held)
        } else {
            side.held
        };
        let Some(old) = from else {
            return (rec, 0);
        };
        let shape = self.shape;
        let (analog, latency) = self.sides[index].analog(driven, shape, Some(old), weight, quality);
        let analog_weight = if shape == DistortionShape::Bitcrush {
            1.0 - weight
        } else {
            weight
        };
        (rec + (analog - rec) * analog_weight, latency)
    }

    #[expect(clippy::too_many_arguments)]
    fn process_side(
        &mut self,
        index: usize,
        input: f64,
        gain: f64,
        asym: f64,
        tape: f64,
        from: Option<DistortionShape>,
        weight: f64,
        hold: bool,
        dither: f64,
        quality: RenderQuality,
    ) -> (f64, f64) {
        let x = {
            let side = &mut self.sides[index];
            let x0 = side.low_cut[0].high(input);
            let x = side.low_cut[1].high(x0);
            let abs_x = x.abs();
            let env_k = if abs_x > side.bias_env {
                self.env_attack
            } else {
                self.env_release
            };
            side.bias_env += (abs_x - side.bias_env) * env_k;
            x
        };
        let driven = biased_input(x, gain, asym, self.sides[index].bias_env);
        let shape = self.shape;
        let crushing =
            shape == DistortionShape::Bitcrush || from == Some(DistortionShape::Bitcrush);
        let (y_shaped, used_lat) = if crushing {
            self.shape_crushed(index, driven, from, weight, hold, dither, quality)
        } else {
            self.sides[index].analog(driven, shape, from, weight, quality)
        };
        let mut y = self.sides[index].dc.process(y_shaped, self.dc_pole);
        if tape > 0.0 {
            let soft = self.sides[index].soften.low(y);
            y += tape * (soft - y);
        }
        // Apply high_cut BEFORE wet power matching so darker EQ stays loudness-balanced.
        if self.high_cut_hz < 19_999.0 {
            let y0 = self.sides[index].high_cut[0].low(y);
            y = self.sides[index].high_cut[1].low(y0);
        }
        if quality == RenderQuality::Offline {
            y = self.align_offline(index, y, used_lat);
            self.clean_delay[index].push(x as f32);
            let clean = f64::from(self.clean_delay[index].at(Self::OFFLINE_LATENCY + 1));
            (clean, y)
        } else {
            (x, y)
        }
    }

    fn update_level_match(&mut self, clean: [f64; 2], wet: [f64; 2]) -> f64 {
        let power = |pair: [f64; 2]| 0.5 * (pair[0] * pair[0] + pair[1] * pair[1]);
        let clean_sq = power(clean);
        let [clean_power, wet_power] = &mut self.detectors;
        let (heard, made) = (
            clean_power.follow(clean_sq, self.level_pole),
            wet_power.follow(power(wet), self.level_pole),
        );
        let target_gain = if !self.level_match {
            1.0
        } else if heard <= GATE_RMS_SQ {
            self.matched_gain
        } else {
            ((heard + POWER_FLOOR) / (made + POWER_FLOOR))
                .sqrt()
                .clamp(1.0 / MATCH_LIMIT, MATCH_LIMIT)
        };
        let delta = target_gain - self.matched_gain;
        self.matched_gain += delta.clamp(-MAX_GAIN_SLEW_PER_SAMPLE, MAX_GAIN_SLEW_PER_SAMPLE);
        self.matched_gain
    }

    pub(super) fn process_with_quality(
        &mut self,
        left: f64,
        right: f64,
        quality: RenderQuality,
    ) -> (f64, f64) {
        let (gain, asym, tape) = (self.gain.next(), self.asym.next(), self.tape.next());
        let active_input = left.abs().max(right.abs()) > 1e-6;
        let from = (self.fade > 1).then_some(self.from);
        let crushing =
            self.shape == DistortionShape::Bitcrush || from == Some(DistortionShape::Bitcrush);
        let (hold, dither) = self.crush_step(active_input, asym, crushing);
        let weight = self.next_fade_weight();
        let (c0, w0) = self.process_side(
            0, left, gain, asym, tape, from, weight, hold, dither, quality,
        );
        let (c1, w1) = self.process_side(
            1, right, gain, asym, tape, from, weight, hold, dither, quality,
        );
        let matched_gain = self.update_level_match([c0, c1], [w0, w1]);
        let matched = [w0 * matched_gain, w1 * matched_gain];
        self.activity = matched[0].abs().max(matched[1].abs());
        let (l, r) = widen(matched[0], matched[1], self.width.next());
        let trim = self.output.next();
        (l * trim, r * trim)
    }
}

#[cfg(test)]
mod tests;
