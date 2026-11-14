//! The saturation pedals: the soft clip, the tube, the maximizer, and the
//! tape. Each runs its curve through `Saturation`.
use super::{REFRESH, Refresh};
use crate::master as settings;
use crate::master::dsp::{
    Biquad, Coefficients, DcBlocker, Delay, Oversampler, Smoothed, db_to_gain, pole,
};
use crate::playback_quality::Oversampling;

/// The same integer stage delay in both playback qualities. Only the
/// saturation curve's oversampling changes; detector and limiter paths do not.
#[derive(Clone, Copy)]
struct Saturation {
    oversampler: Oversampler,
    delay: Delay<{ Oversampler::LATENCY }>,
    quality: Oversampling,
}

impl Saturation {
    fn new(quality: Oversampling) -> Self {
        Self {
            oversampler: Oversampler::ZERO,
            delay: Delay::ZERO,
            quality,
        }
    }

    fn clear(&mut self) {
        *self = Self::new(self.quality);
    }

    #[inline]
    fn process(&mut self, sample: f32, curve: impl FnMut(f32) -> f32) -> f32 {
        match self.quality {
            Oversampling::FourX => self.oversampler.process(sample, curve),
            Oversampling::Off => {
                let mut curve = curve;
                self.delay.push(curve(sample))
            }
        }
    }
}

/// Unity below `knee`. Above it the curve approaches `ceiling` as tanh
/// does, with the same slope and curvature where it starts to bend. A knee
/// at the ceiling clips there.
fn soft_clip(sample: f32, knee: f32, ceiling: f32) -> f32 {
    let magnitude = sample.abs();
    if magnitude <= knee {
        return sample;
    }
    let room = ceiling - knee;
    let shaped = if room > 1e-6 {
        knee + room * ((magnitude - knee) / room).tanh()
    } else {
        ceiling
    };
    shaped.copysign(sample)
}

// Soft clip -------------------------------------------------------------------------

/// Rounds peaks off below a ceiling. The amount sets how far below the
/// ceiling the curve starts to bend.
#[derive(Clone, Copy)]
pub(in crate::master) struct SoftClip {
    oversamplers: [Saturation; 2],
    knee: Smoothed,
    ceiling: Smoothed,
}
impl SoftClip {
    pub(in crate::master) const LATENCY: usize = Oversampler::LATENCY;

    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        Self {
            oversamplers: [Saturation::new(quality); 2],
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
        self.oversamplers.iter_mut().for_each(Saturation::clear);
        self.knee.settle();
        self.ceiling.settle();
    }
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (knee, ceiling) = (self.knee.next() as f32, self.ceiling.next() as f32);
        let curve = |sample| soft_clip(sample, knee, ceiling);
        (
            self.oversamplers[0].process(left, curve),
            self.oversamplers[1].process(right, curve),
        )
    }
}

// Tube ----------------------------------------------------------------------------------

/// Bias of the tube curve: how much sooner positive swings flatten.
const TUBE_BIAS: f32 = 0.3;

/// `tanh(g x) / (g (1 + b tanh(g x)))`. Positive swings flatten sooner than
/// negative ones, which gives mostly even harmonics, and dividing by the
/// drive keeps quiet passages at their level. At zero drive it is the
/// identity.
fn tube(sample: f32, drive: f32) -> f32 {
    if drive < 1e-4 {
        return sample;
    }
    let shaped = (drive * sample).tanh();
    shaped / (drive * (1.0 + TUBE_BIAS * shaped))
}

/// Asymmetric saturation, blended with the dry signal. The asymmetry
/// leaves an offset that a 5 Hz high-pass removes.
#[derive(Clone, Copy)]
pub(in crate::master) struct Tube {
    oversamplers: [Saturation; 2],
    dry: [Delay<{ Oversampler::LATENCY }>; 2],
    blockers: [DcBlocker; 2],
    /// The drive control, 0 to 10; the curve's gain is `4 (drive / 10)²`.
    drive: Smoothed,
    /// The wet share, 0 to 1.
    mix: Smoothed,
    blocker_pole: f64,
}
impl Tube {
    pub(in crate::master) const LATENCY: usize = Oversampler::LATENCY;

    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        Self {
            oversamplers: [Saturation::new(quality); 2],
            dry: [Delay::ZERO; 2],
            blockers: [DcBlocker::ZERO; 2],
            drive: Smoothed::new(0.0),
            mix: Smoothed::new(1.0),
            blocker_pole: pole(1.0 / (2.0 * std::f64::consts::PI * 5.0)),
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::Tube) {
        self.drive.set(f64::from(settings.drive));
        self.mix.set(f64::from(settings.mix) / 100.0);
    }
    pub(in crate::master) fn clear(&mut self) {
        self.oversamplers.iter_mut().for_each(Saturation::clear);
        self.dry = [Delay::ZERO; 2];
        self.blockers = [DcBlocker::ZERO; 2];
        self.drive.settle();
        self.mix.settle();
    }
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let drive = self.drive.next() / 10.0;
        let gain = (4.0 * drive * drive) as f32;
        let mix = self.mix.next() as f32;
        let mut channel = |index: usize, sample: f32| {
            let wet = self.oversamplers[index].process(sample, |x| tube(x, gain));
            let wet = self.blockers[index].process(f64::from(wet), self.blocker_pole) as f32;
            let dry = self.dry[index].push(sample);
            dry + (wet - dry) * mix
        };
        (channel(0, left), channel(1, right))
    }
}

// Maximizer ---------------------------------------------------------------------------------

/// The maximizer's output ceiling, in dBFS.
const MAXIMIZER_CEILING_DB: f64 = -1.0;

/// Raises the level by a quarter of a decibel per percent of boost into a
/// soft clipper with a fixed ceiling, so the sound gets louder while its
/// peaks stay put. The curve bends from 1 dB plus the boost below the
/// ceiling.
#[derive(Clone, Copy)]
pub(in crate::master) struct Maximizer {
    oversamplers: [Saturation; 2],
    drive: Smoothed,
    knee: Smoothed,
}
impl Maximizer {
    pub(in crate::master) const LATENCY: usize = Oversampler::LATENCY;

    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        Self {
            oversamplers: [Saturation::new(quality); 2],
            drive: Smoothed::new(1.0),
            knee: Smoothed::new(1.0),
        }
    }
    pub(in crate::master) fn set(&mut self, settings: &settings::Maximizer) {
        let drive = 0.25 * f64::from(settings.boost);
        self.drive.set(db_to_gain(drive));
        self.knee
            .set(db_to_gain(MAXIMIZER_CEILING_DB - 1.0 - drive));
    }
    pub(in crate::master) fn clear(&mut self) {
        self.oversamplers.iter_mut().for_each(Saturation::clear);
        self.drive.settle();
        self.knee.settle();
    }
    pub(in crate::master) fn process(&mut self, left: f32, right: f32) -> (f32, f32) {
        let (drive, knee) = (self.drive.next() as f32, self.knee.next() as f32);
        let ceiling = db_to_gain(MAXIMIZER_CEILING_DB) as f32;
        let curve = |sample| soft_clip(sample, knee, ceiling);
        (
            self.oversamplers[0].process(left * drive, curve),
            self.oversamplers[1].process(right * drive, curve),
        )
    }
}

// Tape ----------------------------------------------------------------------------------------

/// Emphasis before the tape curve and its inverse after it, so treble
/// saturates sooner than bass, as on tape.
const EMPHASIS: (f64, f64, f64) = (3000.0, 0.7, 6.0);
/// Head bump centres at 15 and 30 inches per second.
const BUMP_HZ: [f64; 2] = [55.0, 35.0];
/// Treble roll-off corners at 15 and 30 inches per second.
const ROLL_OFF_HZ: [f64; 2] = [10_000.0, 16_000.0];

/// `tanh(g x) / g`: symmetric, so it adds odd harmonics, at unity gain for
/// quiet passages.
fn tape(sample: f32, drive: f32) -> f32 {
    if drive < 1e-4 {
        return sample;
    }
    (drive * sample).tanh() / drive
}

/// Tape saturation with pre- and de-emphasis. The drive also sets how much
/// low-end head bump and treble roll-off the tape adds; speed moves both.
/// At zero drive the tape adds nothing.
#[derive(Clone, Copy)]
pub(in crate::master) struct Tape {
    oversamplers: [Saturation; 2],
    /// Emphasis, de-emphasis, head bump, and roll-off.
    filters: [Biquad; 4],
    drive: Smoothed,
    /// From 0 at 15 inches per second to 1 at 30.
    speed: Smoothed,
    refresh: Refresh,
}
impl Tape {
    pub(in crate::master) const LATENCY: usize = Oversampler::LATENCY;

    #[cfg(test)]
    pub(in crate::master) fn new() -> Self {
        Self::with_quality(Oversampling::FourX)
    }

    pub(in crate::master) fn with_quality(quality: Oversampling) -> Self {
        let (hz, slope, db) = EMPHASIS;
        Self {
            oversamplers: [Saturation::new(quality); 2],
            filters: [
                Biquad::new(Coefficients::high_shelf(hz, slope, db)),
                Biquad::new(Coefficients::high_shelf(hz, slope, -db)),
                Biquad::FLAT,
                Biquad::FLAT,
            ],
            drive: Smoothed::new(0.0),
            speed: Smoothed::new(1.0),
            refresh: Refresh(REFRESH),
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
        self.oversamplers.iter_mut().for_each(Saturation::clear);
        self.filters.iter_mut().for_each(Biquad::clear);
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
        let gain = (5.0 * drive * drive) as f32;
        let mut channel = |index: usize, sample: f32| {
            let emphasized = self.filters[0].process(index, f64::from(sample)) as f32;
            let saturated = self.oversamplers[index].process(emphasized, |x| tape(x, gain));
            let mut sample = f64::from(saturated);
            for filter in &mut self.filters[1..] {
                sample = filter.process(index, sample);
            }
            sample as f32
        };
        (channel(0, left), channel(1, right))
    }
}

#[cfg(test)]
mod tests;
