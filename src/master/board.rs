//! The board: the state of every pedal, and one frame's way through the
//! pedals that are on, in the board's order.
use super::booster::{self, clamp16};
use super::{MasterChain, PedalKind, dsp, pedals};

/// Playback keeps this delay even as pedals switch, so the timeline and
/// both sides of a crossfade stay aligned. Export uses only active delays.
pub(super) const LIVE_LATENCY: usize =
    4 * dsp::Oversampler::LATENCY + pedals::Multiband::LATENCY + pedals::Limiter::LATENCY;

/// A stereo frame between two pedals.
#[derive(Clone, Copy)]
enum Frame {
    /// Normalized floats, as the mix and the mastering pedals use.
    Audio(f32, f32),
    /// 16-bit samples, as the Booster pedals take them.
    Integers(i32, i32),
    /// 16-bit samples as floats, as the Booster equaliser gives them.
    Floats(f32, f32),
}
impl Frame {
    /// Leaving the Booster pedals, samples are clamped to 16 bits, as at the
    /// end of the original's stereo path.
    fn audio(self) -> (f32, f32) {
        match self {
            Self::Audio(left, right) => (left, right),
            _ => {
                let (left, right) = self.integers();
                (
                    f32::from(clamp16(left)) / 32768.0,
                    f32::from(clamp16(right)) / 32768.0,
                )
            }
        }
    }
    /// An integer becomes a float exactly; a float becomes an integer by
    /// truncation, and a normalized float by rounding to 16 bits.
    fn integers(self) -> (i32, i32) {
        match self {
            Self::Audio(left, right) => (
                i32::from(crate::audio::float_to_i16(left)),
                i32::from(crate::audio::float_to_i16(right)),
            ),
            Self::Integers(left, right) => (left, right),
            Self::Floats(left, right) => (left as i32, right as i32),
        }
    }
    fn floats(self) -> (f32, f32) {
        match self {
            Self::Floats(left, right) => (left, right),
            _ => {
                let (left, right) = self.integers();
                (left as f32, right as f32)
            }
        }
    }
}

/// The state of every pedal and the settings it runs with.
#[derive(Clone, Copy)]
pub(super) struct Board {
    pub(super) chain: MasterChain,
    pub(super) live: bool,
    pub(super) padding: [dsp::Delay<LIVE_LATENCY>; 2],
    /// Whether any pedal is on.
    pub(super) active: bool,
    pub(super) equalizer: booster::Equalizer,
    pub(super) compressor: booster::Compressor,
    pub(super) wide: booster::Wide,
    soft_clip: pedals::SoftClip,
    tube: pedals::Tube,
    trim_eq: pedals::TrimEq,
    pub(super) bus_compressor: pedals::BusCompressor,
    maximizer: pedals::Maximizer,
    lift_eq: pedals::LiftEq,
    tape: pedals::Tape,
    imaging: pedals::Imaging,
    multiband: pedals::Multiband,
    pub(super) limiter: pedals::Limiter,
    /// Each keyed pedal's key, in `PedalKind::KEYED` order, delayed by the
    /// pedals that run before it, so that it lines up with the sound.
    key_delays: [[dsp::Delay<LIVE_LATENCY>; 2]; 4],
    key_lags: [usize; 4],
}
/// A frame of each keyed pedal's key, in `PedalKind::KEYED` order: `None`
/// for a pedal that listens to its own input.
pub(super) type KeyFrame = [Option<(f32, f32)>; 4];
impl Board {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::with_quality(crate::playback_quality::Oversampling::FourX)
    }

    pub(super) fn with_quality(quality: crate::playback_quality::Oversampling) -> Self {
        Self {
            chain: MasterChain::default(),
            live: false,
            padding: [dsp::Delay::ZERO; 2],
            key_delays: [[dsp::Delay::ZERO; 2]; 4],
            key_lags: [0; 4],
            active: false,
            equalizer: booster::Equalizer::new(),
            compressor: booster::Compressor::at_load(),
            wide: booster::Wide::new(),
            soft_clip: pedals::SoftClip::with_quality(quality),
            tube: pedals::Tube::with_quality(quality),
            trim_eq: pedals::TrimEq::new(),
            bus_compressor: pedals::BusCompressor::new(),
            maximizer: pedals::Maximizer::with_quality(quality),
            lift_eq: pedals::LiftEq::new(),
            tape: pedals::Tape::with_quality(quality),
            imaging: pedals::Imaging::new(),
            multiband: pedals::Multiband::new(),
            limiter: pedals::Limiter::new(),
        }
    }
    /// Take new settings. A pedal that switches on starts from a clear
    /// state, as the original's enable calls do; other changes keep the
    /// state, and the mastering pedals glide to their new settings.
    pub(super) fn apply(&mut self, chain: &MasterChain) {
        let old = self.chain;
        // Leaving bypass switches on every pedal that is on.
        let switched_on = |kind| chain.runs(kind) && !old.runs(kind);
        if switched_on(PedalKind::BoosterEq) {
            self.equalizer.clear();
        }
        if chain.booster_eq.bands != old.booster_eq.bands {
            self.equalizer.set(&chain.booster_eq.bands);
        }
        if switched_on(PedalKind::BoosterCompressor) {
            self.compressor.clear();
        }
        let compressor = &chain.booster_compressor;
        self.compressor
            .set(compressor.drive, compressor.speed, compressor.gain);
        if switched_on(PedalKind::BoosterWide) {
            self.wide.clear();
        }
        self.wide.set(chain.booster_wide.spread);

        self.soft_clip.set(&chain.soft_clip);
        self.tube.set(&chain.tube);
        self.trim_eq.set(&chain.trim_eq);
        self.bus_compressor.set(&chain.bus_compressor);
        self.maximizer.set(&chain.maximizer);
        self.lift_eq.set(&chain.lift_eq);
        self.tape.set(&chain.tape);
        self.imaging.set(&chain.imaging);
        self.multiband.set(&chain.multiband);
        self.limiter.set(&chain.limiter);
        for kind in PedalKind::ALL {
            if switched_on(kind) {
                self.clear(kind);
            }
        }
        self.chain = *chain;
        self.active = PedalKind::ALL.into_iter().any(|kind| chain.runs(kind));
        let mut lag = 0;
        for kind in chain.order.into_iter().filter(|kind| chain.runs(*kind)) {
            if let Some(index) = PedalKind::KEYED.iter().position(|keyed| *keyed == kind) {
                self.key_lags[index] = lag;
            }
            lag += kind.latency();
        }
    }
    /// Where each keyed pedal finds its key among `keys`: `None` for its
    /// own input or a pedal that does not run, `Some(None)` for silence.
    pub(super) fn key_slots(&self, keys: &crate::ducker::Keys) -> [Option<Option<usize>>; 4] {
        PedalKind::KEYED.map(|kind| {
            self.chain
                .runs(kind)
                .then(|| keys.slot(self.chain.key(kind)))
                .flatten()
        })
    }
    /// Clear a mastering pedal's state and move it straight to its settings.
    fn clear(&mut self, kind: PedalKind) {
        match kind {
            PedalKind::SoftClip => self.soft_clip.clear(),
            PedalKind::Tube => self.tube.clear(),
            PedalKind::TrimEq => self.trim_eq.clear(),
            PedalKind::BusCompressor => self.bus_compressor.clear(),
            PedalKind::Maximizer => self.maximizer.clear(),
            PedalKind::LiftEq => self.lift_eq.clear(),
            PedalKind::Tape => self.tape.clear(),
            PedalKind::Imaging => self.imaging.clear(),
            PedalKind::Multiband => self.multiband.clear(),
            PedalKind::Limiter => self.limiter.clear(),
            PedalKind::BoosterEq | PedalKind::BoosterCompressor | PedalKind::BoosterWide => {}
        }
    }
    /// Begin as a board that has run with its settings for a while.
    pub(super) fn settle(&mut self) {
        self.compressor.settle();
        for kind in PedalKind::ALL {
            self.clear(kind);
        }
    }
    /// Whether new settings switch or move a pedal, or bypass the board.
    pub(super) fn rewired(&self, chain: &MasterChain) -> bool {
        chain.bypass != self.chain.bypass
            || chain.order != self.chain.order
            || PedalKind::ALL
                .into_iter()
                .any(|kind| chain.is_on(kind) != self.chain.is_on(kind))
    }
    #[inline]
    pub(super) fn process(&mut self, left: f32, right: f32, keys: KeyFrame) -> (f32, f32) {
        let mut delayed = [None; 4];
        for (index, key) in keys.into_iter().enumerate() {
            // Keep time while a pedal follows its own input too. Otherwise
            // selecting a track again replays key audio from before the switch.
            let (key_left, key_right) = key.unwrap_or((0.0, 0.0));
            let lag = self.key_lags[index];
            let [delay_left, delay_right] = &mut self.key_delays[index];
            let frame = (
                delay_left.push_delayed(key_left, lag),
                delay_right.push_delayed(key_right, lag),
            );
            delayed[index] = key.map(|_| frame);
        }
        let (left, right) = self.run(Frame::Audio(left, right), delayed).audio();
        if self.live {
            let padding = LIVE_LATENCY - self.chain.latency();
            (
                self.padding[0].push_delayed(left, padding),
                self.padding[1].push_delayed(right, padding),
            )
        } else {
            (left, right)
        }
    }
    /// One frame through the pedals that are on, in order. As in
    /// `FX_RenderStereo`, the Booster equaliser works on floats and the other
    /// Booster pedals on integers; the samples change kind only where two
    /// neighbouring pedals differ. In the original's order the Booster pedals
    /// are the library's stereo path.
    /// `keys` holds each keyed pedal's key, already lined up with the frame.
    fn run(&mut self, mut frame: Frame, keys: KeyFrame) -> Frame {
        let [booster_key, compressor_key, multiband_key, limiter_key] = keys;
        for kind in self.chain.order {
            if !self.chain.runs(kind) {
                continue;
            }
            frame = match kind {
                PedalKind::BoosterEq => {
                    let (left, right) = frame.floats();
                    Frame::Floats(
                        self.equalizer.process(0, left),
                        self.equalizer.process(1, right),
                    )
                }
                PedalKind::BoosterCompressor => {
                    let (left, right) = frame.integers();
                    let key = booster_key.map(|(key_left, key_right)| {
                        (
                            i32::from(crate::audio::float_to_i16(key_left)),
                            i32::from(crate::audio::float_to_i16(key_right)),
                        )
                    });
                    let (left, right) = self.compressor.process_keyed(left, right, key);
                    Frame::Integers(left, right)
                }
                PedalKind::BoosterWide => {
                    let (left, right) = frame.integers();
                    let (left, right) = self.wide.process(left, right);
                    Frame::Integers(left, right)
                }
                _ => {
                    let (left, right) = frame.audio();
                    let (left, right) = match kind {
                        PedalKind::SoftClip => self.soft_clip.process(left, right),
                        PedalKind::Tube => self.tube.process(left, right),
                        PedalKind::TrimEq => self.trim_eq.process(left, right),
                        PedalKind::BusCompressor => {
                            self.bus_compressor
                                .process_keyed(left, right, compressor_key)
                        }
                        PedalKind::Maximizer => self.maximizer.process(left, right),
                        PedalKind::LiftEq => self.lift_eq.process(left, right),
                        PedalKind::Tape => self.tape.process(left, right),
                        PedalKind::Imaging => self.imaging.process(left, right),
                        PedalKind::Multiband => {
                            self.multiband.process_keyed(left, right, multiband_key)
                        }
                        _ => self.limiter.process_keyed(left, right, limiter_key),
                    };
                    Frame::Audio(left, right)
                }
            };
        }
        frame
    }
}

#[cfg(test)]
impl Board {
    pub(super) fn settled(chain: &MasterChain) -> Self {
        let mut board = Self::new();
        board.apply(chain);
        board.settle();
        board
    }
    /// One frame of 16-bit samples through the pedals, then the clamp.
    pub(super) fn process_integers(&mut self, left: i32, right: i32) -> (i16, i16) {
        let (left, right) = self.run(Frame::Integers(left, right), [None; 4]).integers();
        (clamp16(left), clamp16(right))
    }
}
