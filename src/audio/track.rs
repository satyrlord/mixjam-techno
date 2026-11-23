//! A track's sound: its clips resolved to voices, and the chain that
//! shapes them before they join the mix.
use super::{
    Mix, Pcm,
    frames::{ChainState, Ramp},
    resample,
};
use crate::{
    automation::{Envelope, FilterSlope},
    engine::FadeCurve,
    eq::Curve,
    fx::{BUSES, RenderQuality},
    master::dsp::{Biquad, Coefficients},
};
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
                    // A bus at zero gets nothing: its sum stays as it is.
                    for (bus, send) in self.sends.iter().enumerate() {
                        if *send > 0.0 {
                            sends[at * 2 * BUSES + bus * 2 + channel] += value * send;
                        }
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
    /// The knob, or else the lane. A lane that never rises above zero
    /// sends nothing, as a knob at zero.
    fn new(knob: f32, lane: &[crate::automation::Point], frames_per_tick: f64) -> Self {
        match Envelope::new(lane, frames_per_tick) {
            Some(envelope) if !envelope.any_positive() => Self::Fixed(0.0),
            Some(envelope) => Self::Moving(envelope),
            None => Self::Fixed(knob),
        }
    }

    fn at(&self, frame: u64) -> f32 {
        match self {
            Self::Fixed(level) => *level,
            Self::Moving(envelope) => envelope.at(frame),
        }
    }

    /// Whether the send ever rises above zero.
    fn sends(&self) -> bool {
        match self {
            Self::Fixed(level) => *level > 0.0,
            Self::Moving(_) => true,
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
    /// The Compression lane, in percent of the compressor: only with the
    /// compressor on.
    compression: Option<Envelope>,
    /// The side's gain: the width as a fraction of 100%.
    width: Option<f32>,
    /// Fourth-order Linkwitz-Riley side high-pass for sub-mono summing.
    width_hpf: Option<[Coefficients; 2]>,
    volume: Option<Envelope>,
    sends: [SendLevel; BUSES],
    filter: Option<Envelope>,
    filter_resonance: f32,
    filter_slope: FilterSlope,
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
        render_quality: RenderQuality,
    ) -> Self {
        let automation = &track.automation;
        let widens = track.widens();
        let width_hpf = (widens && track.mono_below_hz > 20.5).then(|| {
            [Coefficients::high_pass(
                f64::from(track.mono_below_hz),
                std::f64::consts::FRAC_1_SQRT_2,
            ); 2]
        });
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
                    crate::drive::Run::with_quality(&track.drive, quality.drive, render_quality),
                    0,
                )
            }),
            compressor: track
                .compressor
                .on
                .then(|| crate::compressor::Run::new(&track.compressor)),
            compression: track
                .compressor
                .on
                .then(|| Envelope::new(&automation.compression, frames_per_tick))
                .flatten(),
            width: widens.then_some(track.width / 100.0),
            width_hpf,
            volume: Envelope::new(&automation.volume, frames_per_tick),
            sends: std::array::from_fn(|bus| {
                SendLevel::new(track.sends[bus], &automation.sends[bus], frames_per_tick)
            }),
            filter: Envelope::new(&automation.filter, frames_per_tick),
            filter_resonance: track.filter_resonance,
            filter_slope: track.filter_slope,
            pan: Envelope::new(&automation.pan, frames_per_tick),
            ducker: None,
        }
    }

    /// Whether the track ever sends to a bus.
    pub(super) fn feeds(&self, bus: usize) -> bool {
        self.sends[bus].sends()
    }

    /// Fill `buffer` with the track's voices from `span.start`, as its
    /// Repeat lane reads them. Frames at or past `span.end` stay silent:
    /// a drive's lookahead must not read sound beyond the playback loop.
    /// `buffer` holds at most `CHUNK` frames: the Repeat taps live on the
    /// stack.
    fn sound(&self, span: Range<u64>, buffer: &mut [f32]) {
        debug_assert!(buffer.len() / 2 <= Self::CHUNK);
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
        level: &Ramp,
        duck: Option<(&[f32], &mut crate::ducker::State)>,
        state: &mut ChainState,
        drives: &mut [crate::drive::State],
    ) {
        let at = span.start;
        self.sound_through_drive(span, buffer, &mut state.eq, drives);
        self.compress(at, buffer, &mut state.compressor);
        if let Some(width) = self.width {
            widen(buffer, width, self.width_hpf, &mut state.width_hpf);
        }
        self.apply_automation(at, buffer, &mut state.filter, level, duck);
    }

    fn sound_through_drive(
        &self,
        span: Range<u64>,
        buffer: &mut [f32],
        eq: &mut crate::eq::Filters,
        drives: &mut [crate::drive::State],
    ) {
        let mut shaped_sound = |span, buffer: &mut [f32]| {
            self.sound(span, buffer);
            if let Some(curve) = &self.eq {
                eq.process(buffer, curve);
            }
        };
        if let Some((run, slot)) = &self.drive {
            // The drive's output follows its input by its latency, so the
            // sound before it runs that far ahead. A history that does not
            // lead up to this block, after a seek or at the start, first
            // takes the frames from `at`, as the EQ starts from silence.
            const LEAD: usize = crate::drive::LATENCY;
            let at = span.start;
            let history = &mut drives[*slot];
            if history.next != Some(at + LEAD as u64) {
                *history = crate::drive::State::new(run);
                let mut lead = [0.0_f32; LEAD * 2];
                shaped_sound(span.clone(), &mut lead);
                run.process(&mut lead, history);
            }
            shaped_sound(at + LEAD as u64..span.end, buffer);
            run.process(buffer, history);
            history.next = Some(at + (LEAD + buffer.len() / 2) as u64);
        } else {
            shaped_sound(span, buffer);
        }
    }

    fn compress(&self, at: u64, buffer: &mut [f32], history: &mut crate::compressor::State) {
        if let Some(compressor) = &self.compressor {
            match &self.compression {
                Some(envelope) => {
                    // The compressor only scales each frame, so the mix of
                    // the dry and the compressed frame is in phase.
                    let mut dry = [0.0_f32; Self::CHUNK * 2];
                    let dry = &mut dry[..buffer.len()];
                    dry.copy_from_slice(buffer);
                    compressor.process(buffer, history);
                    for (index, (wet, dry)) in buffer
                        .as_chunks_mut::<2>()
                        .0
                        .iter_mut()
                        .zip(dry.as_chunks::<2>().0)
                        .enumerate()
                    {
                        let mix =
                            envelope.at(at + index as u64) / crate::automation::COMPRESSION_FULL;
                        // At 100% the frame stays exactly as compressed.
                        if mix >= 1.0 {
                            continue;
                        }
                        for channel in 0..2 {
                            wet[channel] = dry[channel] + (wet[channel] - dry[channel]) * mix;
                        }
                    }
                }
                None => compressor.process(buffer, history),
            }
        }
    }

    fn apply_automation(
        &self,
        at: u64,
        buffer: &mut [f32],
        filter: &mut crate::automation::Filter,
        level: &Ramp,
        mut duck: Option<(&[f32], &mut crate::ducker::State)>,
    ) {
        for index in 0..buffer.len() / 2 {
            let frame = at + index as u64;
            if let Some(envelope) = &self.filter {
                filter.process_with(
                    &mut buffer[index * 2..index * 2 + 2],
                    envelope.at(frame),
                    Mix::SAMPLE_RATE as f32,
                    self.filter_resonance,
                    self.filter_slope,
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
        self.shape(span, buffer, level, duck, state, drives);
        for (sum, value) in out[offset * 2..(offset + count) * 2]
            .iter_mut()
            .zip(buffer.iter())
        {
            *sum += value;
        }
        let Some(sends) = sends else {
            return;
        };
        // A bus that the track never sends to gets nothing: its sum stays.
        for (bus, send) in self
            .sends
            .iter()
            .enumerate()
            .filter(|(_, send)| send.sends())
        {
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

/// Scale the side of interleaved stereo frames by `width` (1 for no change,
/// 0 folds to mono) and optionally high-pass only the side signal with an LR4
/// filter (`2x` cascaded 2nd-order Butterworth HPF) while preserving Mid.
fn widen(
    frames: &mut [f32],
    width: f32,
    hpf: Option<[Coefficients; 2]>,
    filters: &mut [Biquad; 2],
) {
    if let Some([c0, c1]) = hpf {
        filters[0].set(c0);
        filters[1].set(c1);
    }
    let width = f64::from(width);
    for frame in frames.as_chunks_mut::<2>().0 {
        let l = f64::from(frame[0]);
        let r = f64::from(frame[1]);
        let mid = 0.5 * (l + r);
        let mut side = 0.5 * (l - r);
        if hpf.is_some() {
            let s1 = filters[0].process(0, side);
            side = filters[1].process(0, s1);
        }
        side *= width;
        *frame = [(mid + side) as f32, (mid - side) as f32];
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::f64::consts::TAU;

    fn sine_stereo(hz_l: f64, hz_r: f64, frames: usize) -> Vec<f32> {
        (0..frames)
            .flat_map(|i| {
                let t = i as f64 / Mix::SAMPLE_RATE as f64;
                [(TAU * hz_l * t).sin() as f32, (TAU * hz_r * t).sin() as f32]
            })
            .collect()
    }

    #[test]
    fn width_lr4_mono_below_correlates_sub_bass_and_preserves_mid() {
        let fc = 120.0;
        let coeffs = [Coefficients::high_pass(fc, std::f64::consts::FRAC_1_SQRT_2); 2];
        // Sub-bass well below 0.6 * fc with decorrelated L/R phases.
        let mut low = (0..Mix::SAMPLE_RATE as usize)
            .flat_map(|i| {
                let t = i as f64 / Mix::SAMPLE_RATE as f64;
                [(TAU * 50.0 * t).sin() as f32, (TAU * 50.0 * t).cos() as f32]
            })
            .collect::<Vec<_>>();
        let expected_mid: Vec<f64> = low
            .as_chunks::<2>()
            .0
            .iter()
            .map(|&[l, r]| 0.5 * (f64::from(l) + f64::from(r)))
            .collect();
        let mut filters = [Biquad::FLAT; 2];
        widen(&mut low, 1.0, Some(coeffs), &mut filters);

        let settle = Mix::SAMPLE_RATE as usize / 2;
        let mut dot = 0.0_f64;
        let mut ll = 0.0_f64;
        let mut rr = 0.0_f64;
        let mut mid_err = 0.0_f64;
        let mut mid_sig = 0.0_f64;
        for (pair, m0) in low.as_chunks::<2>().0[settle..]
            .iter()
            .zip(&expected_mid[settle..])
        {
            let l = f64::from(pair[0]);
            let r = f64::from(pair[1]);
            dot += l * r;
            ll += l * l;
            rr += r * r;
            let m1 = 0.5 * (l + r);
            mid_err += (m1 - m0) * (m1 - m0);
            mid_sig += m0 * m0;
        }
        let corr = dot / (ll * rr).sqrt();
        let mid_null_db = 10.0 * ((mid_err + 1e-30) / (mid_sig + 1e-30)).log10();
        assert!(corr >= 0.97, "expected corr >= 0.97 at 50 Hz, got {corr}");
        assert!(
            mid_null_db < -100.0,
            "Mid must stay bit-accurate, null = {mid_null_db:.1} dB"
        );

        // Stereo separation above crossover stays intact (LR4 allpass phase shift
        // moves waveforms slightly while preserving L and R magnitudes).
        let mut high = sine_stereo(1000.0, 2200.0, Mix::SAMPLE_RATE as usize);
        let ref_high = high.clone();
        filters = [Biquad::FLAT; 2];
        widen(&mut high, 1.0, Some(coeffs), &mut filters);
        let rms_ch = |buf: &[f32], ch: usize| {
            let mut sum = 0.0_f64;
            let mut count = 0usize;
            for pair in buf.as_chunks::<2>().0[settle..].iter() {
                let v = f64::from(pair[ch]);
                sum += v * v;
                count += 1;
            }
            (sum / count as f64).sqrt()
        };
        for ch in 0..2 {
            let diff = (rms_ch(&high, ch) - rms_ch(&ref_high, ch)).abs() / rms_ch(&ref_high, ch);
            assert!(
                diff < 0.02,
                "ch {ch} high stereo magnitude changed by {diff}"
            );
        }
    }
}
