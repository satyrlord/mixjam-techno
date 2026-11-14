//! The audio that a player reads in blocks, and the histories that each
//! reader keeps for itself.
use super::Pcm;
use crate::{
    automation::Filter, engine::Project, eq::Filters, fx::ReturnLanes, master::MasterControl,
};
use std::sync::Arc;

/// Stateful processing belongs to a reader, never to the shared audio data.
pub struct RenderState {
    pub(super) chains: [ChainState; crate::engine::TRACK_COUNT],
    /// The key tracks' own filter histories, apart from their mixed ones.
    pub(super) keys: [ChainState; crate::engine::MAX_KEYS],
    /// Each chain's ducker, in the order of the mix's chains.
    pub(super) duckers: [crate::ducker::State; crate::engine::TRACK_COUNT],
    /// The last block's key tracks for the master pedals and the FX buses,
    /// as `Keys::audio` holds them.
    pub(super) key_audio: Vec<f32>,
    pub(super) next_frame: Option<u64>,
    /// Each track's glide to its level, as the last block left it.
    pub(super) levels: [Ramp; crate::engine::TRACK_COUNT],
    /// The glide of the master gain inside those levels, in both channels.
    pub(super) master_gain: Ramp,
    /// Playback's live levels. Without them the mix plays the levels it was
    /// prepared with.
    pub(super) live: Option<Arc<MasterControl>>,
    /// The loop that playback wraps through: a read from its start right
    /// after its end is a loop pass.
    pub(super) loop_region: Option<[u64; 2]>,
    pub(super) compressor_epoch: Option<u64>,
    /// The drives' histories, in the order of the mix's drive slots.
    /// Playback allocates them before the device reads the mix.
    pub(super) drives: Vec<crate::drive::State>,
}

/// How far behind the frame where playback kept a compressor's history a
/// new playback start may begin and still carry on from it. Playback renders
/// ahead of what it has played by up to a block and the master pedals'
/// delay. The cursor that a new start begins at is at most one tick (345
/// frames at 120 BPM) and, after a pause, one window update (about 30 ms)
/// behind what has played. Together that is less than 3300 frames.
pub(super) const CARRY_FRAMES: u64 = 4096;

/// The frames over which a track's level glides to a moved fader, pan, or
/// master gain during playback: about 23 ms, so the move does not click.
const GLIDE_FRAMES: u64 = 1024;

/// Each track's left and right level: fader, polarity, pan, and the master
/// gain. A track whose pan lane has points leaves its pan knob out: the lane
/// pans it frame by frame in its chain.
pub(crate) fn track_levels(project: &Project) -> [[f32; 2]; crate::engine::TRACK_COUNT] {
    std::array::from_fn(|index| {
        let track = &project.tracks[index];
        let gain = track.effective_gain() * project.master_gain;
        let pan = if track.automation.pan.is_empty() {
            track.pan
        } else {
            0.0
        };
        crate::automation::pan_gains(pan).map(|side| gain * side)
    })
}

/// A track's left and right level: a straight glide over `GLIDE_FRAMES`
/// from the level it had reached when its target last changed, so a moved
/// fader does not click. The glide runs on across playback's blocks.
#[derive(Clone, Copy)]
pub(super) struct Ramp {
    /// The first frame of the glide, which already moves one step.
    first_frame: u64,
    frames: f32,
    from: [f32; 2],
    pub(super) to: [f32; 2],
}

impl Ramp {
    pub(super) const UNITY: Self = Self::still([1.0; 2]);

    pub(super) const fn still(level: [f32; 2]) -> Self {
        Self {
            first_frame: 0,
            frames: 1.0,
            from: level,
            to: level,
        }
    }

    pub(super) fn at(&self, frame: u64, channel: usize) -> f32 {
        let (from, to) = (self.from[channel], self.to[channel]);
        if from == to {
            return to;
        }
        let along = ((frame - self.first_frame) as f32 + 1.0) / self.frames;
        if along >= 1.0 {
            return to;
        }
        from + (to - from) * along
    }

    /// Carry the glide into a block from `first_frame`. A new target starts
    /// a new glide from the level reached before that frame; a playback that
    /// does not continue from the last block starts at the target.
    pub(super) fn advance(&mut self, first_frame: u64, continues: bool, target: [f32; 2]) {
        if !continues {
            *self = Self::still(target);
        } else if target != self.to {
            let reached = if first_frame > self.first_frame {
                [0, 1].map(|channel| self.at(first_frame - 1, channel))
            } else {
                self.from
            };
            *self = Self {
                first_frame,
                frames: GLIDE_FRAMES as f32,
                from: reached,
                to: target,
            };
        } else if first_frame.saturating_sub(self.first_frame) as f32 >= self.frames {
            // The glide is over: hold the target.
            self.from = self.to;
        }
    }
}

/// The histories of one track's chain: its EQ, its compressor, and its
/// automation filter.
#[derive(Clone, Copy, Default)]
pub(super) struct ChainState {
    pub(super) eq: Filters,
    pub(super) compressor: crate::compressor::State,
    pub(super) filter: Filter,
}

impl Default for RenderState {
    fn default() -> Self {
        Self {
            chains: [ChainState::default(); crate::engine::TRACK_COUNT],
            keys: [ChainState::default(); crate::engine::MAX_KEYS],
            duckers: [crate::ducker::State::default(); crate::engine::TRACK_COUNT],
            key_audio: Vec::new(),
            next_frame: None,
            levels: [Ramp::still([0.0; 2]); crate::engine::TRACK_COUNT],
            master_gain: Ramp::still([0.0; 2]),
            live: None,
            loop_region: None,
            compressor_epoch: None,
            drives: Vec::new(),
        }
    }
}

/// Audio that a player reads in blocks: decoded PCM or a mix made on demand.
pub trait Frames: Send + Sync {
    fn channels(&self) -> u16;
    fn sample_rate(&self) -> u32;
    fn frames(&self) -> u64;
    /// Fill `out` with interleaved samples from `first_frame` onward.
    /// Samples past the end are silent.
    fn read(&self, first_frame: u64, out: &mut [f32]);
    /// The tempo of a song mix, whose sends feed the FX buses.
    fn tempo(&self) -> Option<f64> {
        None
    }
    /// Read a block with history private to this playback or export. A mix
    /// fills `out` with the dry stereo mix, not yet held to full scale: the
    /// master processor holds it after the board's trim. With `sends` it
    /// fills `sends` with the FX buses' feeds, `2 * BUSES` interleaved
    /// channels a frame. False, with `sends` untouched, when nothing in the
    /// block sends.
    fn read_block(
        &self,
        first_frame: u64,
        out: &mut [f32],
        _sends: Option<&mut [f32]>,
        _state: &mut RenderState,
    ) -> bool {
        self.read(first_frame, out);
        false
    }
    /// The key tracks of the first `frames` frames that the last
    /// `read_block` with `state` read, for the master pedals and the FX
    /// buses.
    fn keys<'a>(&'a self, _state: &'a RenderState, _frames: usize) -> crate::ducker::Keys<'a> {
        crate::ducker::Keys::NONE
    }
    /// Allocate the history that `read_block` needs in `state`, so that the
    /// audio thread does not allocate it.
    fn prepare_state(&self, _state: &mut RenderState) {}
    /// The FX buses' return lanes of a song mix, if any bus has one.
    fn return_lanes(&self) -> Option<&ReturnLanes> {
        None
    }
}

impl Frames for Pcm {
    fn channels(&self) -> u16 {
        self.channels
    }
    fn sample_rate(&self) -> u32 {
        self.sample_rate
    }
    fn frames(&self) -> u64 {
        Pcm::frames(self)
    }
    fn read(&self, first_frame: u64, out: &mut [f32]) {
        let start = (first_frame as usize)
            .saturating_mul(self.channels as usize)
            .min(self.samples.len());
        let available = (self.samples.len() - start).min(out.len());
        out[..available].copy_from_slice(&self.samples[start..start + available]);
        out[available..].fill(0.0);
    }
}
