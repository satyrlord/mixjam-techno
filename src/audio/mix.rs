//! The prepared mix. It holds the voices and the track chains and mixes
//! any range of the song on demand, so a song can be as long as the
//! timeline.
use super::{
    Frames, RenderState,
    frames::{CARRY_FRAMES, ChainState, track_levels},
    track::{TrackChain, Voice},
};
use crate::{
    engine::Project,
    fx::{BUSES, ReturnLanes},
};

/// A project prepared for rendering. It holds the decoded sources and mixes
/// any range of the song on demand, so playback and export need no buffer of
/// the whole song and a song can be as long as the timeline.
pub struct Mix {
    pub(super) quality: crate::playback_quality::PlaybackQuality,
    pub(super) frames: u64,
    pub(super) voices: Vec<Voice>,
    /// The tracks with their EQ, drive, or compressor on, a width other
    /// than 100%, with automation, or with a ducker on or that key one,
    /// which hold their own voices.
    pub(super) chains: Vec<TrackChain>,
    /// The key tracks of the duckers, the master pedals, and the FX buses,
    /// in key slot order, each in a chain of its own that only measures it.
    pub(super) keys: Vec<TrackChain>,
    /// The track of each key slot, and whether the master pedals or the FX
    /// buses listen to any, so that the mix keeps their audio.
    pub(super) key_tracks: Vec<usize>,
    pub(super) side: bool,
    /// The track levels the mix was prepared with.
    pub(super) levels: [[f32; 2]; crate::engine::TRACK_COUNT],
    /// Pan ownership belongs to this prepared mix, including while a new
    /// lane edit is preparing. Live knob levels must use the same ownership.
    pub(super) automated_pan: [bool; crate::engine::TRACK_COUNT],
    /// The master gain inside those levels. A track ducker hears its key
    /// without it, so the master gain does not change how far it ducks.
    pub(super) master_gain: f32,
    pub(super) bpm: f64,
    /// Whether any voice sends to each FX bus, and to any.
    pub(super) fed: [bool; BUSES],
    pub(super) sends: bool,
    /// The drives among the keys and the chains, each with its slot.
    pub(super) drives: usize,
    /// The FX buses' return lanes, when any bus has one.
    pub(super) returns: Option<ReturnLanes>,
}

impl Mix {
    pub const SAMPLE_RATE: u32 = 44_100;
    pub fn frames(&self) -> u64 {
        self.frames
    }

    /// The runtime quality used by this prepared playback mix.
    pub fn quality(&self) -> crate::playback_quality::PlaybackQuality {
        self.quality
    }

    /// Whether the mix was prepared with the project's track levels.
    /// Playback reads them live; an export needs them in the mix.
    pub fn has_levels(&self, project: &Project) -> bool {
        self.levels == track_levels(project)
    }

    /// The return lanes for a block that starts at song frame `first_frame`.
    /// The song's frames go on counting through the buses' tails.
    pub(super) fn lanes(&self, first_frame: u64) -> crate::fx::Lanes<'_> {
        self.returns
            .as_ref()
            .map(|lanes| (lanes, first_frame, None))
    }

    /// The key tracks that the master pedals and the FX buses listen to, for
    /// the first `frames` frames of the block that `state` last mixed. None
    /// when nothing listens.
    pub(super) fn key_block<'a>(
        &'a self,
        state: &'a RenderState,
        frames: usize,
    ) -> crate::ducker::Keys<'a> {
        if !self.side || frames == 0 {
            return crate::ducker::Keys::NONE;
        }
        crate::ducker::Keys {
            tracks: &self.key_tracks,
            audio: &state.key_audio[..frames * 2 * self.key_tracks.len()],
        }
    }

    /// Mix voices into `out`, and into `sends` when given and any voice sends.
    /// `only` mixes one track alone. Runs on the audio thread during
    /// playback: no allocation and no locks.
    pub(super) fn mix(
        &self,
        first_frame: u64,
        out: &mut [f32],
        mut sends: Option<&mut [f32]>,
        state: &mut RenderState,
        only: Option<usize>,
    ) {
        if state.compressor_epoch.is_none() {
            state.compressor_epoch = state.live.as_ref().map(|live| live.compressor_epoch());
        }
        let continues = state.next_frame == Some(first_frame);
        if !continues {
            // Playback's compressors carry on where playback continues: a
            // loop pass keeps the history from the loop's end, and a new
            // playback just behind where the last one kept its history,
            // after a pause or for a mix prepared again such as for each
            // step of a compressor dial, starts from that history. Starting
            // without reduction would play the track up to its full
            // reduction louder until the attack caught up. Any other start,
            // and the song's start, which follows silence as in export,
            // begins from silence like the other histories. A key chain
            // carries its track's history, so the key matches the track.
            let carries = first_frame > 0;
            let wraps = state
                .loop_region
                .is_some_and(|[start, end]| first_frame == start && state.next_frame == Some(end));
            for history in state.chains.iter_mut().chain(&mut state.keys) {
                *history = ChainState {
                    compressor: if wraps {
                        history.compressor
                    } else {
                        Default::default()
                    },
                    ..ChainState::default()
                };
            }
            state.duckers.fill(crate::ducker::State::default());
            for drive in &mut state.drives {
                drive.next = None;
            }
            if carries
                && !wraps
                && let Some(live) = &state.live
            {
                for (chain, history) in (self.chains.iter().zip(&mut state.chains))
                    .chain(self.keys.iter().zip(&mut state.keys))
                {
                    if chain.compressor.is_some()
                        && let Some((kept, at)) = live.kept_compressor(chain.track)
                        && (first_frame..=first_frame + CARRY_FRAMES).contains(&at)
                    {
                        history.compressor = kept;
                    }
                }
            }
        }
        let (targets, master_gain) = state
            .live
            .as_ref()
            .and_then(|live| live.levels(&self.automated_pan))
            .unwrap_or_else(|| {
                if state.next_frame.is_some() {
                    (state.levels.map(|ramp| ramp.to), state.master_gain.to[0])
                } else {
                    (self.levels, self.master_gain)
                }
            });
        for (ramp, target) in state.levels.iter_mut().zip(targets) {
            ramp.advance(first_frame, continues, target);
        }
        let levels = state.levels;
        out.fill(0.0);
        if let Some(sends) = sends.as_deref_mut() {
            sends.fill(0.0);
        }
        // Playback allocated the drives before the device read the mix.
        if state.drives.len() < self.drives {
            state.drives.resize(self.drives, Default::default());
        }
        let included = |track: usize| only.is_none_or(|only| only == track);
        for voice in self.voices.iter().filter(|voice| included(voice.track)) {
            voice.add(first_frame, out, sends.as_deref_mut(), &levels[voice.track]);
        }
        // Chunk by chunk, so that each ducker reads its key over the same
        // frames. The keys are measured whether or not `only` mixes them.
        // A ducker's key level leaves out the master gain; the key audio for
        // the master pedals and the FX buses keeps it, as their input does.
        // The master gain glides as the track levels do, so the key stays
        // free of it while a live master gain moves.
        state
            .master_gain
            .advance(first_frame, continues, [master_gain; 2]);
        let mastered = state.master_gain;
        let frames = out.len() / 2;
        let end = state.loop_region.map_or(u64::MAX, |[_, end]| end);
        let stride = 2 * self.keys.len();
        if self.side && state.key_audio.len() < frames * stride {
            state.key_audio.resize(frames * stride, 0.0);
        }
        for offset in (0..frames).step_by(TrackChain::CHUNK) {
            let count = TrackChain::CHUNK.min(frames - offset);
            let at = first_frame + offset as u64;
            let mut keys = [[0.0_f32; TrackChain::CHUNK]; crate::engine::MAX_KEYS];
            for (slot, ((chain, history), key)) in self
                .keys
                .iter()
                .zip(&mut state.keys)
                .zip(&mut keys)
                .enumerate()
            {
                let mut buffer = [0.0_f32; TrackChain::CHUNK * 2];
                let buffer = &mut buffer[..count * 2];
                chain.shape(
                    at..end,
                    buffer,
                    history,
                    &mut state.drives,
                    &levels[chain.track],
                    None,
                );
                for (index, (level, &[left, right])) in
                    key.iter_mut().zip(buffer.as_chunks::<2>().0).enumerate()
                {
                    let master_gain = mastered.at(at + index as u64, 0);
                    *level = if master_gain > 0.0 {
                        left.abs().max(right.abs()) / master_gain
                    } else {
                        0.0
                    };
                    if self.side {
                        let at = (offset + index) * stride + 2 * slot;
                        state.key_audio[at..at + 2].copy_from_slice(&[left, right]);
                    }
                }
            }
            for ((chain, history), ducker) in self
                .chains
                .iter()
                .zip(&mut state.chains)
                .zip(&mut state.duckers)
            {
                if included(chain.track) {
                    let duck = chain.ducker.map(|(_, slot)| (&keys[slot][..count], ducker));
                    chain.add(
                        at..end,
                        offset,
                        count,
                        out,
                        sends.as_deref_mut(),
                        history,
                        &mut state.drives,
                        &levels[chain.track],
                        duck,
                    );
                }
            }
        }
        let next_frame = first_frame + frames as u64;
        if let Some(live) = &state.live {
            for (chain, history) in self.chains.iter().zip(&state.chains) {
                if chain.compressor.is_some() && included(chain.track) {
                    live.keep_compressor(
                        chain.track,
                        history.compressor,
                        next_frame,
                        state.compressor_epoch.unwrap(),
                    );
                }
            }
        }
        state.next_frame = Some(next_frame);
    }
}

impl Frames for Mix {
    fn channels(&self) -> u16 {
        2
    }
    fn sample_rate(&self) -> u32 {
        Self::SAMPLE_RATE
    }
    fn frames(&self) -> u64 {
        self.frames
    }
    /// A read that starts the filters from silence and holds the mix to full
    /// scale, as no board follows it. Frames past the song's end are silent.
    /// Playback and export call `read_block` with their own `RenderState`
    /// and control their tail processing instead.
    fn read(&self, first_frame: u64, out: &mut [f32]) {
        let frames = self
            .frames
            .saturating_sub(first_frame)
            .min((out.len() / 2) as u64) as usize;
        let (sound, silence) = out.split_at_mut(frames * 2);
        if !sound.is_empty() {
            self.read_block(first_frame, sound, None, &mut RenderState::default());
            hold_to_full_scale(sound);
        }
        silence.fill(0.0);
    }
    fn read_block(
        &self,
        first_frame: u64,
        out: &mut [f32],
        sends: Option<&mut [f32]>,
        state: &mut RenderState,
    ) -> bool {
        let want_sends = sends.is_some();
        self.mix(
            first_frame,
            out,
            if self.sends { sends } else { None },
            state,
            None,
        );
        self.sends && want_sends
    }
    fn keys<'a>(&'a self, state: &'a RenderState, frames: usize) -> crate::ducker::Keys<'a> {
        self.key_block(state, frames)
    }
    fn return_lanes(&self) -> Option<&ReturnLanes> {
        self.returns.as_ref()
    }
    fn prepare_state(&self, state: &mut RenderState) {
        state.drives.resize(self.drives, Default::default());
    }
    fn tempo(&self) -> Option<f64> {
        Some(self.bpm)
    }
}

/// Hold every sample to full scale. Saturation is the reconstruction
/// policy; the original mixer's rounding and resampling remain to be
/// measured. The master processor does this after the board's trim.
pub(crate) fn hold_to_full_scale(samples: &mut [f32]) {
    for sample in samples {
        *sample = sample.clamp(-1.0, 1.0);
    }
}
