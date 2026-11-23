//! The master saturation pedals: soft clip, tube, maximizer, and tape.
//! Each runs ADAA2 in f64 through 8x Kaiser FIR offline and fast live
//! oversampling (4x FIR or 1x ADAA2) with matched integer latency.
#[cfg(test)]
use super::sat_adaa::LIVE_SAT_LATENCY;
use super::sat_adaa::{
    DryDelay, EnvFollower, MAXIMIZER_CEILING_DB, MasterSaturation, OFFLINE_SAT_LATENCY,
    PEAK_GUARD_LATENCY, SatCurve, TAPE_DYNAMIC_BIAS, TAPE_EMPHASIS_DB, TAPE_EMPHASIS_HZ,
    TAPE_EMPHASIS_SLOPE, TUBE_DYNAMIC_BIAS, tape_self_erase,
};
use super::{REFRESH, Refresh};
use crate::fx::oversample::RenderQuality;
use crate::master as settings;
use crate::master::dsp::{
    Biquad, Coefficients, DcBlocker, Delay, LookaheadGain, Smoothed, TruePeak, cascade, db_to_gain,
    pole,
};
use crate::playback_quality::Oversampling;

const BUMP_HZ: [f64; 2] = [55.0, 35.0];
const ROLL_OFF_HZ: [f64; 2] = [10_000.0, 16_000.0];

// Soft clip -------------------------------------------------------------------------

/// Rounds peaks off below a ceiling. The amount sets how far below the
/// ceiling the curve starts to bend.
#[derive(Clone, Copy)]
pub(in crate::master) struct SoftClip {
    oversamplers: [MasterSaturation; 2],
    knee: Smoothed,
    ceiling: Smoothed,
}

impl SoftClip {
    #[cfg(test)]
    pub(in crate::master) const LIVE_LATENCY: usize = LIVE_SAT_LATENCY;
    #[cfg(test)]
    pub(in crate::master) const OFFLINE_LATENCY: usize = OFFLINE_SAT_LATENCY;

    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    #[cfg(test)]
    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        Self::with_render_quality(RenderQuality::Live, quality)
    }

    pub(in crate::master) fn with_render_quality(
        render_quality: RenderQuality,
        live_os: Oversampling,
    ) -> Self {
        Self {
            oversamplers: [MasterSaturation::new(render_quality, live_os); 2],
            knee: Smoothed::new(1.0),
            ceiling: Smoothed::new(1.0),
        }
    }

    pub(in crate::master) fn set(&mut self, settings: &settings::SoftClip) {
        let ceiling = f64::from(settings.ceiling);
        self.ceiling.set(db_to_gain(ceiling));
        self.knee
            .set(db_to_gain(ceiling - f64::from(settings.amount)));
    }

    pub(in crate::master) fn clear(&mut self) {
        self.oversamplers
            .iter_mut()
            .for_each(MasterSaturation::clear);
        self.knee.settle();
        self.ceiling.settle();
    }

    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let curve = SatCurve::SoftClip {
            knee: self.knee.next(),
            ceiling: self.ceiling.next(),
        };
        (
            self.oversamplers[0].process(left, curve),
            self.oversamplers[1].process(right, curve),
        )
    }
}

// Tube ----------------------------------------------------------------------------------

/// Asymmetric envelope-coupled triode saturation blended with dry audio.
/// A 15 Hz high-pass removes residual bias offset.
#[derive(Clone, Copy)]
pub(in crate::master) struct Tube {
    oversamplers: [MasterSaturation; 2],
    dry: [DryDelay; 2],
    envelopes: [EnvFollower; 2],
    blockers: [DcBlocker; 2],
    drive: Smoothed,
    mix: Smoothed,
    blocker_pole: f64,
}

impl Tube {
    #[cfg(test)]
    pub(in crate::master) const LATENCY: usize = LIVE_SAT_LATENCY;

    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    #[cfg(test)]
    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        Self::with_render_quality(RenderQuality::Live, quality)
    }

    pub(in crate::master) fn with_render_quality(
        render_quality: RenderQuality,
        live_os: Oversampling,
    ) -> Self {
        Self {
            oversamplers: [MasterSaturation::new(render_quality, live_os); 2],
            dry: [DryDelay::new(render_quality); 2],
            envelopes: [EnvFollower::new(); 2],
            blockers: [DcBlocker::ZERO; 2],
            drive: Smoothed::new(0.0),
            mix: Smoothed::new(1.0),
            blocker_pole: pole(1.0 / (2.0 * std::f64::consts::PI * 15.0)),
        }
    }

    pub(in crate::master) fn set(&mut self, settings: &settings::Tube) {
        self.drive.set(f64::from(settings.drive));
        self.mix.set(f64::from(settings.mix) / 100.0);
    }

    pub(in crate::master) fn clear(&mut self) {
        self.oversamplers
            .iter_mut()
            .for_each(MasterSaturation::clear);
        self.dry.iter_mut().for_each(DryDelay::clear);
        self.envelopes.iter_mut().for_each(EnvFollower::clear);
        self.blockers = [DcBlocker::ZERO; 2];
        self.drive.settle();
        self.mix.settle();
    }

    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let drive = self.drive.next() / 10.0;
        let curve = SatCurve::Tube {
            gain: 4.0 * drive * drive,
        };
        let mix = self.mix.next() as f32;
        let mut channel = |index: usize, sample: f32| {
            let env = self.envelopes[index].follow(f64::from(sample));
            let biased = (f64::from(sample) + TUBE_DYNAMIC_BIAS * drive * env.min(1.0)) as f32;
            let wet = self.oversamplers[index].process(biased, curve);
            let wet = self.blockers[index].process(f64::from(wet), self.blocker_pole) as f32;
            let dry = self.dry[index].push(sample);
            dry + (wet - dry) * mix
        };
        (channel(0, left), channel(1, right))
    }
}

// Maximizer ---------------------------------------------------------------------------------

pub(in crate::master) const MAXIMIZER_OFFLINE_LATENCY: usize =
    OFFLINE_SAT_LATENCY + PEAK_GUARD_LATENCY;
const GUARD_LOOKAHEAD: usize = PEAK_GUARD_LATENCY + 1 - 2 * TruePeak::DELAY;
const GUARD_WINDOW: usize = GUARD_LOOKAHEAD + 1 + 2 * TruePeak::DELAY;

/// Stereo-linked gain for the true peaks of the reconstructed signal.
#[derive(Clone, Copy)]
struct PeakGuard {
    detector: [TruePeak; 2],
    delay: [Delay<PEAK_GUARD_LATENCY>; 2],
    gain: LookaheadGain<f32, GUARD_LOOKAHEAD, GUARD_WINDOW>,
    ceiling: f32,
}

impl PeakGuard {
    fn new() -> Self {
        Self {
            detector: [TruePeak::ZERO; 2],
            delay: [Delay::ZERO; 2],
            gain: LookaheadGain::new(GUARD_WINDOW, pole(0.250) as f32),
            // Leave room for reconstruction filters other than this detector.
            ceiling: db_to_gain(-1.20) as f32,
        }
    }

    fn clear(&mut self) {
        self.detector = [TruePeak::ZERO; 2];
        self.delay = [Delay::ZERO; 2];
        self.gain.clear();
    }

    #[inline]
    fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let peak = self.detector[0]
            .push(left)
            .max(self.detector[1].push(right));
        let need = if peak > self.ceiling {
            self.ceiling / peak
        } else {
            1.0
        };
        let gain = self.gain.push(need) as f32;
        (
            self.delay[0].push(left) * gain,
            self.delay[1].push(right) * gain,
        )
    }
}

/// Raises loudness into an ADAA2 soft clipper and limits reconstruction peaks.
#[derive(Clone, Copy)]
pub(in crate::master) struct Maximizer {
    oversamplers: [MasterSaturation; 2],
    guard: PeakGuard,
    drive: Smoothed,
    knee: Smoothed,
    ceiling: f64,
}

impl Maximizer {
    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    #[cfg(test)]
    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        Self::with_render_quality(RenderQuality::Live, quality)
    }

    pub(in crate::master) fn with_render_quality(
        render_quality: RenderQuality,
        live_os: Oversampling,
    ) -> Self {
        Self {
            oversamplers: [MasterSaturation::guarded(render_quality, live_os); 2],
            guard: PeakGuard::new(),
            drive: Smoothed::new(1.0),
            knee: Smoothed::new(1.0),
            ceiling: db_to_gain(MAXIMIZER_CEILING_DB),
        }
    }

    pub(in crate::master) fn set(&mut self, settings: &settings::Maximizer) {
        let drive = 0.25 * f64::from(settings.boost);
        self.drive.set(db_to_gain(drive));
        self.knee
            .set(db_to_gain(MAXIMIZER_CEILING_DB - 1.0 - drive));
    }

    pub(in crate::master) fn clear(&mut self) {
        self.oversamplers
            .iter_mut()
            .for_each(MasterSaturation::clear);
        self.guard.clear();
        self.drive.settle();
        self.knee.settle();
    }

    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let drive = self.drive.next() as f32;
        let curve = SatCurve::SoftClip {
            knee: self.knee.next(),
            ceiling: self.ceiling,
        };
        let l = self.oversamplers[0].process_with_linear_response(left * drive, curve, true);
        let r = self.oversamplers[1].process_with_linear_response(right * drive, curve, true);
        self.guard.process(l, r)
    }
}

// Tape ----------------------------------------------------------------------------------------

/// Tape saturation with pre/de-emphasis, ADAA2, dynamic bias, hysteresis-like
/// HF self-erasure, head bump, and speed-dependent treble roll-off.
#[derive(Clone, Copy)]
pub(in crate::master) struct Tape {
    oversamplers: [MasterSaturation; 2],
    filters: [Biquad; 4],
    envelopes: [EnvFollower; 2],
    blockers: [DcBlocker; 2],
    erase_state: [f64; 2],
    drive: Smoothed,
    speed: Smoothed,
    refresh: Refresh,
    blocker_pole: f64,
}

impl Tape {
    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    #[cfg(test)]
    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        Self::with_render_quality(RenderQuality::Live, quality)
    }

    pub(in crate::master) fn with_render_quality(
        render_quality: RenderQuality,
        live_os: Oversampling,
    ) -> Self {
        Self {
            oversamplers: [MasterSaturation::new(render_quality, live_os); 2],
            filters: [
                Biquad::new(Coefficients::high_shelf(
                    TAPE_EMPHASIS_HZ,
                    TAPE_EMPHASIS_SLOPE,
                    TAPE_EMPHASIS_DB,
                )),
                Biquad::new(Coefficients::high_shelf(
                    TAPE_EMPHASIS_HZ,
                    TAPE_EMPHASIS_SLOPE,
                    -TAPE_EMPHASIS_DB,
                )),
                Biquad::FLAT,
                Biquad::FLAT,
            ],
            envelopes: [EnvFollower::new(); 2],
            blockers: [DcBlocker::ZERO; 2],
            erase_state: [0.0; 2],
            drive: Smoothed::new(0.0),
            speed: Smoothed::new(1.0),
            refresh: Refresh(REFRESH),
            blocker_pole: pole(1.0 / (2.0 * std::f64::consts::PI * 15.0)),
        }
    }

    pub(in crate::master) fn set(&mut self, settings: &settings::Tape) {
        self.drive.set(f64::from(settings.drive));
        self.speed.set(if settings.speed == 15 { 0.0 } else { 1.0 });
    }

    fn design(&mut self) {
        let color = (self.drive.value() / 10.0).max(0.0).sqrt();
        let speed = self.speed.value();
        let between = |[slow, fast]: [f64; 2]| slow + (fast - slow) * speed;
        self.filters[2].set(Coefficients::peaking(between(BUMP_HZ), 1.0, 2.0 * color));
        self.filters[3].set(Coefficients::high_shelf(
            between(ROLL_OFF_HZ),
            0.7,
            -3.0 * color,
        ));
    }

    pub(in crate::master) fn clear(&mut self) {
        self.oversamplers
            .iter_mut()
            .for_each(MasterSaturation::clear);
        self.filters.iter_mut().for_each(Biquad::clear);
        self.envelopes.iter_mut().for_each(EnvFollower::clear);
        self.blockers = [DcBlocker::ZERO; 2];
        self.erase_state = [0.0; 2];
        self.drive.settle();
        self.speed.settle();
        self.design();
    }

    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let gliding = !(self.drive.settled() && self.speed.settled());
        if gliding {
            self.drive.next();
            self.speed.next();
        }
        if self
            .refresh
            .due(gliding, self.drive.settled() && self.speed.settled())
        {
            self.design();
        }
        let drive = self.drive.value() / 10.0;
        let curve = SatCurve::Tape {
            gain: 5.0 * drive * drive,
        };
        let mut channel = |index: usize, sample: f32| {
            let x = f64::from(sample);
            let env = self.envelopes[index].follow(x);
            let emphasized = self.filters[0].process(index, x);
            let biased = (emphasized + TAPE_DYNAMIC_BIAS * drive * env.min(1.0)) as f32;
            let saturated = self.oversamplers[index].process(biased, curve);
            let clean_dc = if drive >= 1e-3 {
                self.blockers[index].process(f64::from(saturated), self.blocker_pole)
            } else {
                f64::from(saturated)
            };
            let erased = if drive >= 1e-3 {
                tape_self_erase(&mut self.erase_state[index], clean_dc, drive, env)
            } else {
                clean_dc
            };
            cascade(&mut self.filters[1..], index, erased) as f32
        };
        (channel(0, left), channel(1, right))
    }
}

#[cfg(test)]
mod tests;
