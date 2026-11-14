//! Recording a take from the input device and saving it as a source.
use super::{Session, TRACK_COUNT, duration_ticks};
use std::path::Path;

impl Session {
    /// Give UI lifecycle tests an active recording without an input device.
    #[cfg(all(test, feature = "gui"))]
    pub(crate) fn start_recording_fixture(&mut self) {
        self.recorder = Some(crate::recording::Recorder::fixture());
    }
    pub(super) fn require_idle_recording(&self) -> Result<(), String> {
        if self.is_recording() {
            Err("Stop recording before you open, close, or start a new mix.".into())
        } else {
            Ok(())
        }
    }
    pub fn start_recording(&mut self, track: usize) -> Result<(), String> {
        if self.is_recording() {
            return Err("Recording has already started.".into());
        }
        if self.recorded_take.is_some() {
            return Err("Save or discard the previous recorded take first.".into());
        }
        if track >= TRACK_COUNT {
            return Err("The track is outside the project.".into());
        }
        let recorder = crate::recording::Recorder::start(self.project().bpm)?;
        self.record_start_tick = self.cursor_tick;
        self.record_track = track;
        self.record_bpm = self.project().bpm;
        self.record_mix_started = false;
        self.recorder = Some(recorder);
        match crate::recording::count_in(self.record_bpm) {
            Ok(pcm) => self.play_pcm(
                pcm,
                "Recording count-in: eight beats. Press Stop to finish.",
            ),
            Err(error) => self.error(error),
        }
        // Render an edited mix during the count-in, so that it starts with the take.
        if self.render_dirty
            && !self.project().clips.is_empty()
            && self.player.is_some()
            && let Err(error) = self.request_render()
        {
            self.error(error);
        }
        Ok(())
    }
    pub(super) fn poll_recording(&mut self) {
        let Some(recorder) = &self.recorder else {
            return;
        };
        let status = recorder.status();
        if let Some(error) = status.error {
            // The input cannot recover. Keep what it captured as the take.
            self.stop();
            self.error(if self.recorded_take.is_some() {
                format!("{error} Recording stopped. The audio captured before that is kept.")
            } else {
                error
            });
            return;
        }
        if status.limit_reached {
            self.stop();
            return;
        }
        if status.count_in_frames_remaining > 0 {
            self.message(format!(
                "Count-in: {:.1} seconds. Recording track {}.",
                status.count_in_frames_remaining as f64 / status.sample_rate as f64,
                self.record_track + 1
            ));
        } else {
            if !self.record_mix_started {
                self.record_mix_started = true;
                self.cursor_tick = self.record_start_tick;
                if !self.project().clips.is_empty() {
                    self.start_playback();
                }
            }
            self.message(format!(
                "Recording track {}: {:.1} seconds. Press Stop to save the take.",
                self.record_track + 1,
                status.captured_frames as f64 / status.sample_rate as f64
            ));
        }
    }
    pub fn preview_recorded_take(&mut self) {
        if let Some(pcm) = self.recorded_take.clone() {
            self.play_shared_pcm(pcm, "Listening to the recorded take.");
        }
    }
    pub fn discard_recorded_take(&mut self) {
        self.recorded_take = None;
    }
    /// Saves the take first. A placement collision retains the saved source in the catalog.
    pub fn save_recorded_take(&mut self, path: &Path) -> Result<Option<u64>, String> {
        let pcm = self
            .recorded_take
            .clone()
            .ok_or("No recorded take is available.")?;
        let id = self.save_pcm(&pcm, path, "Wave", self.record_bpm)?;
        let ticks = duration_ticks(pcm.frames(), pcm.sample_rate, self.record_bpm);
        let placed = self
            .editor
            .place(id, self.record_track, self.record_start_tick, ticks);
        self.recorded_take = None;
        match placed {
            Ok(clip_id) => {
                self.changed();
                Ok(Some(clip_id))
            }
            Err(error) => {
                self.error(format!(
                    "The take is saved in Wave. Cannot place it on the armed track: {error}"
                ));
                Ok(None)
            }
        }
    }
}
