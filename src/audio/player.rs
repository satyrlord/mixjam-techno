//! The player: the transport over the device's output, and the playback
//! that it plays.
use super::{Frames, device_mixer::Playback, flight, output, source::PlaybackSource};
use crate::master::MasterControl;
use std::{path::Path, sync::Arc};

pub struct Player {
    output: output::Output,
    preview_gain: f32,
    previewing: bool,
    current: Option<Playback>,
    start_frame: u64,
    total_frames: u64,
    loop_region: Option<[u64; 2]>,
}

impl Player {
    pub fn new() -> Result<Self, String> {
        Self::new_with_buffer(crate::options::PlaybackBufferTarget::Ms40)
    }

    pub fn new_with_buffer(target: crate::options::PlaybackBufferTarget) -> Result<Self, String> {
        let output = output::Output::open_with_target(target)
            .map_err(|e| format!("Cannot open the audio output: {e}"))?;
        Ok(Self {
            output,
            preview_gain: 1.0,
            previewing: false,
            current: None,
            start_frame: 0,
            total_frames: 0,
            loop_region: None,
        })
    }
    pub fn output_status(&self) -> String {
        self.output.status()
    }

    /// Save the last half minute that the device played, with `session`
    /// describing the song, to `data_root/diagnostics/<local time>/`.
    pub(crate) fn save_capture(
        &self,
        reason: &str,
        data_root: &Path,
        session: serde_json::Value,
    ) -> Result<flight::Saved, String> {
        self.output.capture(reason, data_root, Some(session))
    }

    pub fn buffer_target_error(
        &self,
        target: crate::options::PlaybackBufferTarget,
    ) -> Option<String> {
        self.output.buffer_target_error(target)
    }

    pub(crate) fn prepare_output(
        &self,
        target: crate::options::PlaybackBufferTarget,
    ) -> Result<output::Output, String> {
        self.output.replacement(target)
    }

    pub(crate) fn adopt_output(&mut self, output: output::Output) {
        self.stop();
        self.output = output;
    }

    /// Attenuate only previews, without changing their source PCM.
    pub fn set_preview_gain(&mut self, gain: f32) {
        self.preview_gain = if gain.is_finite() {
            gain.clamp(0.0, 1.0)
        } else {
            1.0
        };
        if self.previewing
            && let Some(current) = &self.current
        {
            current.set_volume(self.preview_gain);
        }
    }
    /// Play validated audio. Callers check decoded PCM before they pass it.
    /// A mix plays through `master`, whose settings can change as it plays.
    pub fn play(
        &mut self,
        audio: Arc<dyn Frames>,
        start_frame: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
    ) -> Result<(), String> {
        self.play_with_quality(
            audio,
            start_frame,
            loop_region,
            master,
            crate::playback_quality::PlaybackQuality::FULL,
        )
    }

    pub fn play_with_quality(
        &mut self,
        audio: Arc<dyn Frames>,
        start_frame: u64,
        loop_region: Option<[u64; 2]>,
        master: Option<Arc<MasterControl>>,
        quality: crate::playback_quality::PlaybackQuality,
    ) -> Result<(), String> {
        let total = audio.frames();
        if !(1..=2).contains(&audio.channels())
            || audio.sample_rate() == 0
            || audio.sample_rate() > 384_000
        {
            return Err("The audio must be mono or stereo with a supported sample rate.".into());
        }
        if total == 0 {
            return Err("There is no audio to play.".into());
        }
        if let Some([a, b]) = loop_region
            && (a >= b || b > total)
        {
            return Err("The audio loop is outside the song.".into());
        }
        let start = if let Some([a, b]) = loop_region {
            if start_frame >= b { a } else { start_frame }
        } else {
            start_frame.min(total)
        };
        let previewing = master.is_none();
        let source = PlaybackSource::with_quality_block(
            audio,
            start,
            loop_region,
            master,
            PlaybackSource::BLOCK_FRAMES,
            quality,
        );
        // The old playback is stopped before the new one is handed over: it
        // plays out the rest of its block, at most 5.8 ms, and renders no
        // other.
        if let Some(old) = self.current.take() {
            old.end();
        }
        let volume = if previewing { self.preview_gain } else { 1.0 };
        self.current = Some(self.output.start(source, volume)?);
        self.previewing = previewing;
        self.start_frame = start;
        self.total_frames = total;
        self.loop_region = loop_region;
        Ok(())
    }
    pub fn pause(&self) {
        if let Some(current) = &self.current {
            current.pause();
        }
    }
    pub fn stop(&mut self) {
        if let Some(current) = self.current.take() {
            current.end();
        }
        self.start_frame = 0;
        self.loop_region = None;
    }
    pub fn is_playing(&self) -> bool {
        self.current.as_ref().is_some_and(Playback::is_playing)
    }
    pub fn position_frames(&self) -> u64 {
        let elapsed = self.current.as_ref().map_or(0, Playback::played_frames);
        let pos = self.start_frame.saturating_add(elapsed);
        if let Some([a, b]) = self.loop_region {
            if pos >= b {
                a + (pos - b) % (b - a)
            } else {
                pos
            }
        } else {
            pos.min(self.total_frames)
        }
    }
}
