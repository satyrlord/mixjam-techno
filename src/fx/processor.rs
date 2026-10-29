//! The return stage: it runs the four buses on the send feeds and adds
//! their returns to the dry mix, for playback and export alike.
use super::parts::{Glide, SafetyLimiter};
use super::{BUSES, FxBus, FxBuses, Module, delay, reverb};
use crate::master::MasterControl;
use std::sync::Arc;

/// The safety limiters' lookahead: 5 ms. Every return is this much late,
/// and the effects shorten their own delays by as much.
pub(crate) const LOOKAHEAD: usize = 220;
/// The longest tail that playback and export let ring after the song: one
/// minute. A delay with feedback above 100% never dies away by itself.
pub(crate) const TAIL_LIMIT: u64 = 60 * 44_100;
/// A cut (Clear Tail or a change of effect) fades the return out over 12 ms.
const CUT: usize = 529;
/// A bus sleeps after two seconds without input or internal activity.
/// Delay buses also wait long enough for any pending repeat to arrive.
const SLEEP_AFTER: usize = 2 * 44_100;
const SILENCE: f64 = 1e-6;

#[derive(Clone, Copy, PartialEq, Eq)]
enum Kind {
    Empty,
    Delay,
    Reverb,
}
fn kind(module: &Module) -> Kind {
    match module {
        Module::Empty => Kind::Empty,
        Module::EchoformDelay(_) => Kind::Delay,
        Module::AetherformReverb(_) => Kind::Reverb,
    }
}

struct Bus {
    delay: delay::Core,
    reverb: reverb::Core,
    kind: Kind,
    settings: FxBus,
    /// The input switch, gliding so a switch does not click.
    input: Glide,
    level: Glide,
    limiter: SafetyLimiter,
    awake: bool,
    quiet: usize,
    /// Frames left of a cut's fade, and whether the module changes after it.
    cut: usize,
    pending: Option<FxBus>,
}
impl Bus {
    fn new() -> Self {
        Self {
            delay: delay::Core::new(),
            reverb: reverb::Core::new(),
            kind: Kind::Empty,
            settings: FxBus::default(),
            input: Glide::new(1.0, 0.003),
            level: Glide::new(0.0, 0.005),
            limiter: SafetyLimiter::new(),
            awake: false,
            quiet: 0,
            cut: 0,
            pending: None,
        }
    }
    /// Take new settings. A different effect waits for a cut of the old
    /// one's tail while it still sounds.
    fn apply(&mut self, settings: &FxBus, bpm: f64, settle: bool) {
        if kind(&settings.module) != self.kind && !settle && self.awake {
            self.pending = Some(*settings);
            if self.cut == 0 {
                self.cut = CUT;
            }
            return;
        }
        // Settings for the effect that sounds now replace any change still waiting.
        self.pending = None;
        if kind(&settings.module) != self.kind {
            self.kind = kind(&settings.module);
            self.silence();
        }
        let compensation = LOOKAHEAD as f64;
        let fresh = settle || !self.awake;
        match &settings.module {
            Module::Empty => {}
            Module::EchoformDelay(delay) => {
                self.delay.set(delay, bpm, compensation);
                if fresh {
                    self.delay.settle();
                }
            }
            Module::AetherformReverb(reverb) => {
                self.reverb.set(reverb, compensation);
                if fresh {
                    self.reverb.settle();
                }
            }
        }
        self.input.set(if settings.powered { 1.0 } else { 0.0 });
        self.level.set(f64::from(settings.return_level));
        if settle {
            self.input.settle();
            self.level.settle();
        }
        self.settings = *settings;
    }
    fn silence(&mut self) {
        self.delay.clear();
        self.reverb.clear();
        self.limiter.clear();
        self.awake = false;
        self.quiet = 0;
    }
    fn clear_tail(&mut self) {
        if self.awake && self.cut == 0 {
            self.cut = CUT;
        }
    }
    /// The ducker's key setting of the bus's effect: 0 is its own input.
    fn duck_key(&self) -> u8 {
        match &self.settings.module {
            Module::Empty => 0,
            Module::EchoformDelay(delay) => delay.duck_key,
            Module::AetherformReverb(reverb) => reverb.duck_key,
        }
    }
    /// `key` is the ducker's key at this frame, or `None` for the bus's input.
    #[inline]
    fn process(&mut self, left: f64, right: f64, key: Option<(f64, f64)>, bpm: f64) -> (f64, f64) {
        let gate = self.input.next();
        let (left, right) = (left * gate, right * gate);
        let fed = left != 0.0 || right != 0.0;
        if !self.awake {
            if !fed || self.kind == Kind::Empty {
                return (0.0, 0.0);
            }
            self.awake = true;
            self.quiet = 0;
        }
        let (wet_l, wet_r) = match self.kind {
            Kind::Empty => (0.0, 0.0),
            Kind::Delay => self.delay.process_keyed(left, right, key),
            Kind::Reverb => self.reverb.process_keyed(left, right, key),
        };
        let mut level = self.level.next();
        if self.cut > 0 {
            level *= self.cut as f64 / CUT as f64;
            self.cut -= 1;
            if self.cut == 0 {
                self.silence();
                if let Some(settings) = self.pending.take() {
                    self.apply(&settings, bpm, false);
                }
                return (0.0, 0.0);
            }
        }
        let out = self
            .limiter
            .process(wet_l * level, wet_r * level, self.settings.limiter);
        let (activity, wait) = match self.kind {
            Kind::Delay => (self.delay.activity(), SLEEP_AFTER + self.delay.silent_gap()),
            Kind::Reverb => (self.reverb.activity(), SLEEP_AFTER),
            Kind::Empty => (0.0, SLEEP_AFTER),
        };
        if fed || activity > SILENCE {
            self.quiet = 0;
        } else {
            self.quiet += 1;
            if self.quiet > wait {
                self.silence();
            }
        }
        out
    }
}

/// Runs the buses over blocks of the mix. Its state carries from one block
/// to the next.
pub struct ReturnsProcessor {
    control: Option<(Arc<MasterControl>, u64, [u64; BUSES])>,
    bpm: f64,
    buses: Box<[Bus; BUSES]>,
}
impl ReturnsProcessor {
    /// A processor that starts with its effects at their settings and
    /// every tail silent.
    pub fn new(settings: &FxBuses, bpm: f64) -> Self {
        let mut buses: Box<[Bus; BUSES]> = Box::new(std::array::from_fn(|_| Bus::new()));
        for (bus, settings) in buses.iter_mut().zip(&settings.buses) {
            bus.apply(settings, bpm, true);
        }
        Self {
            control: None,
            bpm,
            buses,
        }
    }
    /// A processor that takes its settings, and Clear Tail requests, from
    /// `control` as they change.
    pub fn live(control: Arc<MasterControl>, bpm: f64) -> Self {
        let mut seen = 0;
        let settings = control
            .poll_returns(&mut seen)
            .unwrap_or_else(|| control.returns());
        let mut processor = Self::new(&settings, bpm);
        let clears = std::array::from_fn(|bus| control.tail_clears(bus));
        processor.control = Some((control, seen, clears));
        processor
    }
    /// Where each bus's ducker finds its key among `keys`.
    fn key_slots(&self, keys: &crate::ducker::Keys) -> [Option<Option<usize>>; BUSES] {
        std::array::from_fn(|bus| keys.slot(self.buses[bus].duck_key()))
    }
    /// Whether any bus still sounds.
    pub fn ringing(&self) -> bool {
        self.buses.iter().any(|bus| bus.awake)
    }
    fn poll(&mut self) {
        let Some((control, seen, clears)) = &mut self.control else {
            return;
        };
        if let Some(settings) = control.poll_returns(seen) {
            for (bus, settings) in self.buses.iter_mut().zip(&settings.buses) {
                bus.apply(settings, self.bpm, false);
            }
        }
        for (index, bus) in self.buses.iter_mut().enumerate() {
            let count = control.tail_clears(index);
            if count != clears[index] {
                clears[index] = count;
                bus.clear_tail();
            }
        }
    }
    /// Add the returns to an interleaved stereo block of the dry mix. The
    /// master processor holds the sum to full scale after the board's trim.
    /// `sends` holds the four buses' feeds, eight interleaved channels a
    /// frame, or is `None` when nothing sends.
    pub fn process(&mut self, block: &mut [f32], sends: Option<&[f32]>) {
        self.process_keyed(block, sends, crate::ducker::Keys::NONE);
    }
    /// `process` with the key tracks of the block, which the duckers whose
    /// key names a track follow.
    pub fn process_keyed(
        &mut self,
        block: &mut [f32],
        sends: Option<&[f32]>,
        keys: crate::ducker::Keys,
    ) {
        self.poll();
        if sends.is_some() || self.ringing() {
            let bpm = self.bpm;
            let slots = self.key_slots(&keys);
            for (frame, out) in block.as_chunks_mut::<2>().0.iter_mut().enumerate() {
                let mut sum = (0.0, 0.0);
                for (index, bus) in self.buses.iter_mut().enumerate() {
                    let (left, right) = sends.map_or((0.0, 0.0), |sends| {
                        let at = frame * 2 * BUSES + index * 2;
                        (f64::from(sends[at]), f64::from(sends[at + 1]))
                    });
                    let key = key_at(&keys, slots[index], frame);
                    let (l, r) = bus.process(left, right, key, bpm);
                    sum.0 += l;
                    sum.1 += r;
                }
                out[0] += sum.0 as f32;
                out[1] += sum.1 as f32;
            }
        }
    }
    /// Each bus's return alone, for an export in parts: `returns` gets the
    /// four returns, eight interleaved channels a frame as `sends` holds the
    /// feeds, without the dry mix and not held to full scale. The buses run
    /// as `process` runs them.
    pub fn split(&mut self, returns: &mut [f32], sends: Option<&[f32]>, keys: crate::ducker::Keys) {
        self.poll();
        returns.fill(0.0);
        if sends.is_none() && !self.ringing() {
            return;
        }
        let bpm = self.bpm;
        let slots = self.key_slots(&keys);
        for (frame, out) in returns
            .as_chunks_mut::<{ 2 * BUSES }>()
            .0
            .iter_mut()
            .enumerate()
        {
            for (index, bus) in self.buses.iter_mut().enumerate() {
                let (left, right) = sends.map_or((0.0, 0.0), |sends| {
                    let at = frame * 2 * BUSES + index * 2;
                    (f64::from(sends[at]), f64::from(sends[at + 1]))
                });
                let key = key_at(&keys, slots[index], frame);
                let (l, r) = bus.process(left, right, key, bpm);
                out[index * 2] = l as f32;
                out[index * 2 + 1] = r as f32;
            }
        }
    }
}

/// A bus's key at a frame of the block: `None` for its own input.
fn key_at(
    keys: &crate::ducker::Keys,
    slot: Option<Option<usize>>,
    frame: usize,
) -> Option<(f64, f64)> {
    slot.map(|slot| {
        let (left, right) = keys.frame(slot, frame);
        (f64::from(left), f64::from(right))
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::fx::{Aetherform, Echoform, TimeMode};

    fn plain_delay() -> Echoform {
        Echoform {
            division_l: crate::fx::Division::Quarter,
            division_r: crate::fx::Division::Quarter,
            ping_pong: false,
            width: 100.0,
            low_cut: 20.0,
            high_cut: 20_000.0,
            mod_depth: 0.0,
            duck_amount: 0.0,
            feedback: 80.0,
            output_db: 0.0,
            character: crate::fx::DelayCharacter::Digital,
            ..Echoform::default()
        }
    }

    #[test]
    fn a_ducker_whose_key_names_a_track_follows_that_track() {
        let delay = Echoform {
            duck_amount: 100.0,
            duck_key: 3,
            ..plain_delay()
        };
        let frames = 88_200;
        let mut sends = vec![0.0_f32; frames * 2 * BUSES];
        for (frame, feed) in sends.chunks_mut(2 * BUSES).enumerate() {
            // About -50 dBFS: too quiet to duck by itself.
            let value = 0.003 * (frame as f32 * 0.05).sin();
            feed[..2].copy_from_slice(&[value, value]);
        }
        let run = |delay: Echoform, key: f32| {
            let mut processor = ReturnsProcessor::new(&buses(Module::EchoformDelay(delay)), 120.0);
            let audio = vec![key; frames * 2];
            let keys = crate::ducker::Keys {
                tracks: &[2],
                audio: &audio,
            };
            let mut block = vec![0.0; frames * 2];
            processor.process_keyed(&mut block, Some(&sends), keys);
            block[frames..]
                .iter()
                .map(|v| f64::from(*v).powi(2))
                .sum::<f64>()
        };
        let quiet = run(delay, 0.0);
        assert!(quiet > 0.0);
        // A key at -6 dBFS ducks the repeats by the full 24 dB.
        assert!(run(delay, 0.5) < quiet * 0.01);
        // The bus's own input ignores the key tracks.
        let own = Echoform {
            duck_key: 0,
            ..delay
        };
        assert_eq!(run(own, 0.5), run(own, 0.0));
    }

    #[test]
    fn all_synced_repeats_stay_on_the_grid() {
        let mut processor =
            ReturnsProcessor::new(&buses(Module::EchoformDelay(plain_delay())), 120.0);
        let out = feed(&mut processor, 70_000, |i| if i == 0 { 0.5 } else { 0.0 });
        for repeat in 1..=3 {
            let expected = repeat * 22_050;
            let at = (expected - 600..expected + 20)
                .max_by(|&a, &b| out[a * 2].abs().total_cmp(&out[b * 2].abs()))
                .unwrap();
            assert!(
                at.abs_diff(expected) <= 2,
                "repeat {repeat}: {at}, expected {expected}"
            );
        }
    }

    #[test]
    fn a_long_note_delay_does_not_sleep_before_its_first_echo() {
        let settings = Echoform {
            division_l: crate::fx::Division::WholeDotted,
            division_r: crate::fx::Division::WholeDotted,
            ..plain_delay()
        };
        let mut processor = ReturnsProcessor::new(&buses(Module::EchoformDelay(settings)), 120.0);
        let out = feed(&mut processor, 140_000, |i| if i == 0 { 0.5 } else { 0.0 });
        assert!(out[264_590..264_620].iter().any(|x| x.abs() > 0.05));
    }

    #[test]
    fn bypass_preserves_the_hidden_tail() {
        let control = MasterControl::new(&crate::master::MasterChain::default());
        let settings = Echoform {
            feedback: 99.0,
            ..plain_delay()
        };
        control.set_returns(&buses(Module::EchoformDelay(settings)));
        let mut processor = ReturnsProcessor::live(control.clone(), 120.0);
        feed(&mut processor, 1000, |i| if i < 100 { 0.5 } else { 0.0 });
        control.set_returns(&buses(Module::EchoformDelay(Echoform {
            bypass: true,
            ..settings
        })));
        feed(&mut processor, 3 * 44_100, |_| 0.0);
        control.set_returns(&buses(Module::EchoformDelay(settings)));
        let out = feed(&mut processor, 44_100, |_| 0.0);
        assert!(out.iter().any(|x| x.abs() > 0.001));
    }

    fn buses(module: Module) -> FxBuses {
        let mut buses = FxBuses::default();
        buses.buses[0].module = module;
        buses
    }

    fn feed(
        processor: &mut ReturnsProcessor,
        frames: usize,
        input: impl Fn(usize) -> f32,
    ) -> Vec<f32> {
        let mut out = Vec::new();
        for start in (0..frames).step_by(1024) {
            let count = 1024.min(frames - start);
            let mut block = vec![0.0; count * 2];
            let mut sends = vec![0.0; count * 2 * BUSES];
            for i in 0..count {
                sends[i * 2 * BUSES] = input(start + i);
                sends[i * 2 * BUSES + 1] = input(start + i);
            }
            processor.process(&mut block, Some(&sends));
            out.extend(block);
        }
        out
    }

    #[test]
    fn an_empty_bus_returns_silence() {
        let mut processor = ReturnsProcessor::new(&FxBuses::default(), 120.0);
        let out = feed(&mut processor, 10_000, |_| 0.5);
        assert!(out.iter().all(|v| *v == 0.0));
        assert!(!processor.ringing());
    }

    #[test]
    fn a_synced_echo_arrives_on_the_beat_despite_the_limiter() {
        let delay = Echoform {
            division_l: crate::fx::Division::Quarter,
            division_r: crate::fx::Division::Quarter,
            ping_pong: false,
            low_cut: 20.0,
            high_cut: 20_000.0,
            mod_depth: 0.0,
            duck_amount: 0.0,
            feedback: 0.0,
            character: crate::fx::DelayCharacter::Digital,
            ..Echoform::default()
        };
        let mut processor = ReturnsProcessor::new(&buses(Module::EchoformDelay(delay)), 125.0);
        let out = feed(&mut processor, 30_000, |i| if i == 0 { 0.5 } else { 0.0 });
        let first = out.chunks(2).position(|f| f[0].abs() > 0.05).unwrap();
        assert!((first as i64 - 21_168).abs() <= 2, "{first}");
    }

    #[test]
    fn the_return_level_scales_and_the_limiter_holds_the_ceiling() {
        let reverb = Aetherform {
            output_db: 12.0,
            duck_amount_percent: 0.0,
            ..Aetherform::default()
        };
        let mut settings = buses(Module::AetherformReverb(reverb));
        let loud = |i: usize| 0.9 * (i as f32 * 0.05).sin();
        let mut processor = ReturnsProcessor::new(&settings, 120.0);
        let out = feed(&mut processor, 44_100, loud);
        let peak = out.iter().fold(0.0_f32, |a, v| a.max(v.abs()));
        assert!(peak <= 0.892, "{peak}");
        settings.buses[0].return_level = 0.0;
        let mut processor = ReturnsProcessor::new(&settings, 120.0);
        let out = feed(&mut processor, 10_000, loud);
        assert!(out.iter().all(|v| *v == 0.0));
    }

    #[test]
    fn power_off_stops_new_input_and_the_tail_rings_out_then_sleeps() {
        let delay = Echoform {
            mode: TimeMode::Free,
            time_ms_l: 100.0,
            time_ms_r: 100.0,
            feedback: 50.0,
            duck_amount: 0.0,
            ..Echoform::default()
        };
        let mut settings = buses(Module::EchoformDelay(delay));
        settings.buses[0].powered = false;
        let mut processor = ReturnsProcessor::new(&settings, 120.0);
        let out = feed(&mut processor, 20_000, |_| 0.5);
        assert!(
            out.iter().all(|v| *v == 0.0),
            "no input reaches a bus that is off"
        );
        settings.buses[0].powered = true;
        let mut processor = ReturnsProcessor::new(&settings, 120.0);
        feed(&mut processor, 2000, |i| if i < 100 { 0.5 } else { 0.0 });
        assert!(processor.ringing());
        feed(&mut processor, 4 * 44_100, |_| 0.0);
        assert!(
            !processor.ringing(),
            "the bus sleeps once its tail has died"
        );
    }

    #[test]
    fn clear_tail_and_a_change_of_effect_cut_the_tail_quickly() {
        let control = MasterControl::new(&crate::master::MasterChain::default());
        let reverb = Aetherform {
            decay_seconds: 20.0,
            ..Aetherform::default()
        };
        control.set_returns(&buses(Module::AetherformReverb(reverb)));
        let mut processor = ReturnsProcessor::live(control.clone(), 120.0);
        feed(&mut processor, 8000, |i| if i < 4000 { 0.5 } else { 0.0 });
        control.clear_tail(0);
        let out = feed(&mut processor, 4096, |_| 0.0);
        let after = out[2 * 1500..].iter().fold(0.0_f32, |a, v| a.max(v.abs()));
        assert_eq!(after, 0.0, "the tail is gone 34 ms after Clear Tail");
        feed(&mut processor, 8000, |i| if i < 4000 { 0.5 } else { 0.0 });
        control.set_returns(&FxBuses::default());
        let out = feed(&mut processor, 4096, |_| 0.0);
        let after = out[2 * 1500..].iter().fold(0.0_f32, |a, v| a.max(v.abs()));
        assert_eq!(after, 0.0);
    }
}
