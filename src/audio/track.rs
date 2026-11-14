//! A track's sound: its clips resolved to voices, and the chain that
//! shapes them before they join the mix.
use super::{
    Mix, Pcm,
    frames::{ChainState, Ramp},
    resample,
};
use crate::{automation::Envelope, engine::FadeCurve, eq::Curve, fx::BUSES};
use std::{ops::Range, sync::Arc};

/// One clip, resolved to output frames and its decoded source.
#[derive(Clone)]
pub(super) struct Voice {
    pub(super) track: usize,
    pub(super) source: Arc<Pcm>,
    pub(super) start: u64,
    pub(super) end: u64,
    pub(super) offset_frames: u32,
    pub(super) step: f64,
    /// The clip gain for the left and right outputs; `None` for a disabled
    /// output. The track's level comes in at mix time.
    pub(super) channel_gains: [Option<f32>; 2],
    /// The track's sends, applied after its level.
    pub(super) sends: [f32; BUSES],
    /// Fade lengths in frames. The fade out ends at `sound_end`, the clip's
    /// end or the earlier end of its source audio.
    pub(super) fade_in: u64,
    pub(super) fade_out: u64,
    pub(super) curves: [FadeCurve; 2],
    pub(super) sound_end: u64,
    /// Play the clip backwards: each frame reads where the forward clip's
    /// mirror frame within `start..end` reads, so the result is a pure
    /// function of the source and never of what played before.
    pub(super) reverse: bool,
    /// The track's Pitch lane, which moves the read at its own speed.
    pub(super) varispeed: Option<Arc<crate::automation::Varispeed>>,
    /// Read the source through the band-limited kernel (`resample`) rather
    /// than in a straight line between two frames. Only a clip that plays
    /// at its source's own rate and tempo, without a Pitch lane, reads
    /// whole frames and needs neither.
    pub(super) band_limited: bool,
}

impl Voice {
    /// The frames on each side of a frame over which a Pitch lane's speed is
    /// averaged to set the band-limited kernel's cutoff there. A jump in the
    /// lane's speed then moves the cutoff over 64 frames rather than at one,
    /// and the cutoff stays a function of the frame alone, so playback from
    /// any frame and in any block size reads what export reads.
    const SPEED_SPAN: u64 = 32;

    /// The kernel's scale at `frame` under the Pitch lane `clock`.
    fn scale_at(clock: &crate::automation::Varispeed, step: f64, frame: u64) -> f32 {
        let (low, high) = (
            frame.saturating_sub(Self::SPEED_SPAN),
            frame + Self::SPEED_SPAN,
        );
        let speed = (clock.at(high) - clock.at(low)) / (high - low) as f64;
        resample::scale(step * speed)
    }

    /// Add the voice at `level` to `out`, stereo frames from `first_frame`,
    /// and to `sends` when given.
    pub(super) fn add(
        &self,
        first_frame: u64,
        out: &mut [f32],
        mut sends: Option<&mut [f32]>,
        level: &Ramp,
    ) {
        let block_end = first_frame + (out.len() / 2) as u64;
        let from = self.start.max(first_frame);
        let to = self.end.min(block_end);
        if from >= to {
            return;
        }
        let source = &*self.source;
        let source_frames = source.frames() as usize;
        let source_channels = source.channels as usize;
        let sending = sends.is_some() && self.sends.iter().any(|send| *send > 0.0);
        let fixed_scale = resample::scale(self.step);
        // The band-limited read hears only the clip's slice of its source,
        // as the straight-line read does: a trim cuts the sound before it.
        let slice = &source.samples
            [(self.offset_frames as usize * source_channels).min(source.samples.len())..];
        // Under a Pitch lane, the read moves by the lane's clock instead of
        // the frames. Reversed, it reaches the clip's offset at its end.
        let origin = self.varispeed.as_ref().map(|clock| {
            if self.reverse {
                clock.at(self.end - 1)
            } else {
                clock.at(self.start)
            }
        });
        for frame in from..to {
            let along = match (&self.varispeed, origin) {
                (Some(clock), Some(origin)) if self.reverse => origin - clock.at(frame),
                (Some(clock), Some(origin)) => clock.at(frame) - origin,
                _ if self.reverse => (self.end - 1 - frame) as f64,
                _ => (frame - self.start) as f64,
            };
            let pos = self.offset_frames as f64 + along * self.step;
            let index = pos.floor() as usize;
            if index >= source_frames {
                // Reversed, the frames past the source's end come first.
                if self.reverse {
                    continue;
                }
                break;
            }
            let read = if self.band_limited {
                let scale = match &self.varispeed {
                    Some(clock) => Self::scale_at(clock, self.step, frame),
                    None => fixed_scale,
                };
                resample::read(slice, source_channels, along * self.step, scale)
            } else {
                let next = (index + 1).min(source_frames - 1);
                let fraction = (pos - index as f64) as f32;
                std::array::from_fn(|ch| {
                    let ch = ch.min(source_channels - 1);
                    let a = source.samples[index * source_channels + ch];
                    let b = source.samples[next * source_channels + ch];
                    a + (b - a) * fraction
                })
            };
            let at = (frame - first_frame) as usize;
            let fade = self.fade(frame);
            for (channel, channel_gain) in self.channel_gains.iter().enumerate() {
                let Some(channel_gain) = channel_gain else {
                    continue;
                };
                let ch = if source_channels == 1 { 0 } else { channel };
                let value = read[ch] * channel_gain * fade * level.at(frame, channel);
                out[at * 2 + channel] += value;
                if sending && let Some(sends) = sends.as_deref_mut() {
                    for (bus, send) in self.sends.iter().enumerate() {
                        sends[at * 2 * BUSES + bus * 2 + channel] += value * send;
                    }
                }
            }
        }
    }

    /// The fade gain at an output frame within the clip.
    pub(super) fn fade(&self, frame: u64) -> f32 {
        let mut gain = 1.0;
        let since = frame - self.start;
        if since < self.fade_in {
            gain = self.curves[0].gain((since as f32 + 0.5) / self.fade_in as f32);
        }
        let left = self.sound_end.saturating_sub(frame);
        if left <= self.fade_out {
            gain *= self.curves[1].gain((left as f32 - 0.5) / self.fade_out as f32);
        }
        gain
    }
}

/// A track send: the knob's fixed amount, or its automation lane.
enum SendLevel {
    Fixed(f32),
    Moving(Envelope),
}

impl SendLevel {
    fn at(&self, frame: u64) -> f32 {
        match self {
            Self::Fixed(level) => *level,
            Self::Moving(envelope) => envelope.at(frame),
        }
    }
}

/// A track with its EQ, drive, or compressor on, a width other than 100%,
/// with automation, with its ducker on, or that keys a ducker. Its voices,
/// which read their sources at its Pitch lane's speed (`Voice::varispeed`),
/// mix into a short buffer, through its Repeat lane, that its EQ, drive,
/// compressor, width, filter, volume, ducker, level and pan, and sends then
/// shape, in that order.
pub(super) struct TrackChain {
    pub(super) track: usize,
    pub(super) voices: Vec<Voice>,
    /// The Repeat lane, which reads the voices at other frames.
    repeat: Option<crate::automation::Repeat>,
    eq: Option<Curve>,
    /// The drive and the slot of its history among the mix's drives. The
    /// chain reads its sound `drive::LATENCY` frames ahead of the drive, so
    /// the drive's output lines up with the song.
    pub(super) drive: Option<(crate::drive::Run, usize)>,
    pub(super) compressor: Option<crate::compressor::Run>,
    /// The side's gain: the width as a fraction of 100%.
    width: Option<f32>,
    volume: Option<Envelope>,
    sends: [SendLevel; BUSES],
    filter: Option<Envelope>,
    /// The pan lane, which replaces the pan knob: the track's level then
    /// holds no pan, and the lane's balance gains apply at each frame.
    pan: Option<Envelope>,
    /// The ducker and the slot of its key among the mix's keys.
    pub(super) ducker: Option<(crate::ducker::Run, usize)>,
}

impl TrackChain {
    /// The stack buffer used to sum voices before frame-by-frame automation.
    pub(super) const CHUNK: usize = 128;

    pub(super) fn new(
        index: usize,
        track: &crate::engine::Track,
        frames_per_tick: f64,
        quality: crate::playback_quality::PlaybackQuality,
    ) -> Self {
        let automation = &track.automation;
        Self {
            track: index,
            voices: Vec::new(),
            repeat: crate::automation::Repeat::new(
                &automation.repeat,
                &automation.repeat_gate,
                frames_per_tick,
            ),
            eq: Curve::of(&track.eq),
            // The mix gives each drive its slot.
            drive: track.drive.on.then(|| {
                (
                    crate::drive::Run::with_quality(&track.drive, quality.drive),
                    0,
                )
            }),
            compressor: track
                .compressor
                .on
                .then(|| crate::compressor::Run::new(&track.compressor)),
            width: track.widens().then_some(track.width / 100.0),
            volume: Envelope::new(&automation.volume, frames_per_tick),
            sends: std::array::from_fn(|bus| {
                Envelope::new(&automation.sends[bus], frames_per_tick)
                    .map_or(SendLevel::Fixed(track.sends[bus]), SendLevel::Moving)
            }),
            filter: Envelope::new(&automation.filter, frames_per_tick),
            pan: Envelope::new(&automation.pan, frames_per_tick),
            ducker: None,
        }
    }

    /// Whether the track ever sends to a bus.
    pub(super) fn feeds(&self, bus: usize) -> bool {
        match &self.sends[bus] {
            SendLevel::Fixed(level) => *level > 0.0,
            SendLevel::Moving(envelope) => envelope.any_positive(),
        }
    }

    /// Fill `buffer` with the track's voices from `span.start`, as its
    /// Repeat lane reads them. Frames at or past `span.end` stay silent:
    /// a drive's lookahead must not read sound beyond the playback loop.
    fn sound(&self, span: Range<u64>, buffer: &mut [f32]) {
        buffer.fill(0.0);
        let count = span
            .end
            .saturating_sub(span.start)
            .min((buffer.len() / 2) as u64) as usize;
        let buffer = &mut buffer[..count * 2];
        let at = span.start;
        let Some(repeat) = &self.repeat else {
            for voice in &self.voices {
                voice.add(at, buffer, None, &Ramp::UNITY);
            }
            return;
        };
        let mut taps = [crate::automation::Tap {
            main: (0, 0.0),
            tail: None,
        }; Self::CHUNK];
        for (index, tap) in taps[..count].iter_mut().enumerate() {
            *tap = repeat.tap(at + index as u64);
        }
        let taps = &taps[..count];
        // Each frame reads the voices at its tap. Runs of frames whose taps
        // keep the same distance from them read one contiguous stretch.
        type Read = fn(&crate::automation::Tap) -> Option<(u64, f32)>;
        let reads: [Read; 2] = [|tap| Some(tap.main), |tap| tap.tail];
        for read in reads {
            let distance =
                |index: usize| read(&taps[index]).map(|(from, _)| from as i64 - index as i64);
            let mut first = 0;
            while first < count {
                let Some(offset) = distance(first) else {
                    first += 1;
                    continue;
                };
                let mut last = first + 1;
                while last < count && distance(last) == Some(offset) {
                    last += 1;
                }
                let mut stretch = [0.0_f32; Self::CHUNK * 2];
                let stretch = &mut stretch[..(last - first) * 2];
                let from = (first as i64 + offset) as u64;
                for voice in &self.voices {
                    voice.add(from, stretch, None, &Ramp::UNITY);
                }
                for (index, pair) in (first..last).zip(stretch.as_chunks::<2>().0) {
                    let weight = read(&taps[index]).map_or(0.0, |(_, weight)| weight);
                    buffer[index * 2] += pair[0] * weight;
                    buffer[index * 2 + 1] += pair[1] * weight;
                }
                first = last;
            }
        }
    }

    /// Fill `buffer`, stereo frames from `span.start`, with the track as it sounds:
    /// its voices through its Repeat lane, EQ, drive, compressor, width,
    /// filter, volume, ducker, level, and pan lane. `drives` holds the
    /// drives' histories, and `duck` the key's level at each frame and the
    /// ducker's history. `span.end` bounds the source reads at the loop end.
    pub(super) fn shape(
        &self,
        span: Range<u64>,
        buffer: &mut [f32],
        state: &mut ChainState,
        drives: &mut [crate::drive::State],
        level: &Ramp,
        mut duck: Option<(&[f32], &mut crate::ducker::State)>,
    ) {
        let at = span.start;
        match &self.drive {
            Some((run, slot)) => {
                // The drive's output follows its input by its latency, so the
                // sound before it runs that far ahead. A history that does not
                // lead up to this block, after a seek or at the start, first
                // takes the frames from `at`, as the EQ starts from silence.
                const LEAD: usize = crate::drive::LATENCY;
                let history = &mut drives[*slot];
                if history.next != Some(at + LEAD as u64) {
                    *history = crate::drive::State::new(run);
                    let mut lead = [0.0_f32; LEAD * 2];
                    self.sound(span.clone(), &mut lead);
                    if let Some(curve) = &self.eq {
                        state.eq.process(&mut lead, curve);
                    }
                    run.process(&mut lead, history);
                }
                self.sound(at + LEAD as u64..span.end, buffer);
                if let Some(curve) = &self.eq {
                    state.eq.process(buffer, curve);
                }
                run.process(buffer, history);
                history.next = Some(at + (LEAD + buffer.len() / 2) as u64);
            }
            None => {
                self.sound(span, buffer);
                if let Some(curve) = &self.eq {
                    state.eq.process(buffer, curve);
                }
            }
        }
        if let Some(compressor) = &self.compressor {
            compressor.process(buffer, &mut state.compressor);
        }
        if let Some(width) = self.width {
            widen(buffer, width);
        }
        for index in 0..buffer.len() / 2 {
            let frame = at + index as u64;
            if let Some(envelope) = &self.filter {
                state.filter.process(
                    &mut buffer[index * 2..index * 2 + 2],
                    envelope.at(frame),
                    Mix::SAMPLE_RATE as f32,
                );
            }
            let mut gain = self.volume.as_ref().map_or(1.0, |envelope| {
                crate::automation::volume_gain(envelope.at(frame))
            });
            if let (Some((run, _)), Some((keys, history))) = (&self.ducker, duck.as_mut()) {
                gain *= run.gain(keys[index], history);
            }
            // The lane moves at every frame, so a fast sweep has no steps.
            let sides = self.pan.as_ref().map_or([1.0; 2], |envelope| {
                crate::automation::pan_gains(envelope.at(frame))
            });
            for (channel, side) in sides.into_iter().enumerate() {
                buffer[index * 2 + channel] =
                    buffer[index * 2 + channel] * gain * level.at(frame, channel) * side;
            }
        }
    }

    /// Add `count` frames of the track, from `span.start`, to `out` and `sends` at
    /// frame `offset` of the block. Runs on the audio thread: the buffer is
    /// on the stack.
    #[allow(clippy::too_many_arguments)]
    pub(super) fn add(
        &self,
        span: Range<u64>,
        offset: usize,
        count: usize,
        out: &mut [f32],
        sends: Option<&mut [f32]>,
        state: &mut ChainState,
        drives: &mut [crate::drive::State],
        level: &Ramp,
        duck: Option<(&[f32], &mut crate::ducker::State)>,
    ) {
        let at = span.start;
        let mut buffer = [0.0_f32; Self::CHUNK * 2];
        let buffer = &mut buffer[..count * 2];
        self.shape(span, buffer, state, drives, level, duck);
        for (sum, value) in out[offset * 2..(offset + count) * 2]
            .iter_mut()
            .zip(buffer.iter())
        {
            *sum += value;
        }
        let Some(sends) = sends else {
            return;
        };
        for (bus, send) in self.sends.iter().enumerate() {
            for index in 0..count {
                let level = send.at(at + index as u64);
                for channel in 0..2 {
                    sends[(offset + index) * 2 * BUSES + bus * 2 + channel] +=
                        buffer[index * 2 + channel] * level;
                }
            }
        }
    }
}

/// Scale the side of interleaved stereo frames by `width`, 1 for no change,
/// and keep the mid: 0 folds the frames to mono.
fn widen(frames: &mut [f32], width: f32) {
    for frame in frames.as_chunks_mut::<2>().0 {
        let mid = 0.5 * (frame[0] + frame[1]);
        let side = 0.5 * (frame[0] - frame[1]) * width;
        *frame = [mid + side, mid - side];
    }
}
