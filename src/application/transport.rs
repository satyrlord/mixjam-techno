//! Playback, preview, the prepared mix that the player reads, and playback captures.
use super::render_worker::RenderRequest;
use super::{MeterReading, Pcm, Session};
use crate::audio::{self, Player};
use std::{path::PathBuf, sync::Arc};

impl Session {
    pub fn playing_mix(&self) -> bool {
        !self.previewing && self.player.as_ref().is_some_and(Player::is_playing)
    }
    /// The master meters of the latest mix playback, which hold their last
    /// reading after it stops.
    pub fn master_meters(&self) -> MeterReading {
        self.master.meters()
    }
    pub fn is_previewing(&self) -> bool {
        self.previewing && self.player.as_ref().is_some_and(Player::is_playing)
    }
    /// Changes when a preview starts or the transport replaces or stops it.
    pub fn preview_revision(&self) -> u64 {
        self.preview_revision
    }

    fn tick_to_frame(&self, tick: u32, rate: u32) -> u64 {
        (tick as f64 * self.editor.project().frames_per_tick(rate)).round() as u64
    }

    pub fn start_playback(&mut self) {
        if self.player.is_none() {
            self.error("Audio output is unavailable. You can still edit and export your mix.");
            return;
        }
        if self.editor.project().clips.is_empty() {
            self.pause();
            self.message("Add a sound to a track before you press Play.");
            return;
        }
        self.wants_playback = true;
        // A queued mix playback supersedes a pending tool audition even before
        // the render worker has produced its first playback buffer.
        self.preview_revision = self.preview_revision.wrapping_add(1);
        if self.render_dirty || self.render.is_none() {
            // The revision may be preparing already, ahead of Play.
            if self.requested_revision != Some(self.revision)
                && let Err(error) = self.request_render()
            {
                self.wants_playback = false;
                self.error(error);
                return;
            }
            self.message("Preparing mix audio.");
            return;
        }
        self.apply_playback_buffer();
    }

    /// Prepare the current revision before Play asks for it, so that Play
    /// does not wait for the sources to load and their pitched copies.
    /// Play itself reports a failure, so a failed revision waits for it.
    pub(super) fn prepare_ahead(&mut self) {
        if self.prepares_ahead
            && !self.wants_playback
            && (self.render_dirty || self.render.is_none())
            && self.requested_revision != Some(self.revision)
            && self.failed_revision != Some(self.revision)
            && !self.project().clips.is_empty()
        {
            // Play reports a stopped worker when it asks again.
            let _ = self.request_render();
        }
    }

    /// Queue a render of the current revision. `poll_render` stores the result.
    pub(super) fn request_render(&mut self) -> Result<(), String> {
        self.render_worker.request(RenderRequest {
            revision: self.revision,
            quality: self.options.playback_quality(),
            project: self.editor.project().clone(),
            library: self.library.clone(),
        })?;
        self.requested_revision = Some(self.revision);
        Ok(())
    }

    pub(super) fn cancel_playback_request(&mut self) {
        self.wants_playback = false;
        self.render_worker.cancel_pending();
        self.requested_revision = None;
    }

    fn apply_playback_buffer(&mut self) {
        let mix = self.render.as_ref().unwrap().clone();
        let rate = audio::Mix::SAMPLE_RATE;
        self.playback_frames_per_tick = self.editor.project().frames_per_tick(rate);
        let start = self.tick_to_frame(self.cursor_tick, rate);
        let region = self
            .editor
            .project()
            .loop_region
            .map(|[a, b]| [self.tick_to_frame(a, rate), self.tick_to_frame(b, rate)]);
        self.master.set(&self.editor.project().master);
        self.master.set_returns(&self.editor.project().fx);
        self.master.set_levels(self.editor.project());
        let master = Some(self.master.clone());
        match self.player.as_mut().unwrap().play_with_quality(
            mix,
            start,
            region,
            master,
            self.options.playback_quality(),
        ) {
            Ok(()) => {
                self.previewing = false;
                self.preview_revision = self.preview_revision.wrapping_add(1);
                self.seek_pending = false;
                self.message("Playing mix.");
            }
            Err(error) => {
                self.wants_playback = false;
                self.error(format!("Cannot start audio: {error}"));
            }
        }
    }

    pub(super) fn poll_render(&mut self) {
        let Some(result) = self.render_worker.poll() else {
            return;
        };
        if result.revision != self.revision {
            return;
        }
        self.requested_revision = None;
        match result.mix {
            Ok(mix) => {
                if self.playing_mix() && !self.seek_pending {
                    self.cursor_tick = (self.player.as_ref().unwrap().position_frames() as f64
                        / self.playback_frames_per_tick)
                        as u32;
                }
                self.render = Some(mix);
                self.render_dirty = false;
                if self.wants_playback {
                    self.apply_playback_buffer();
                }
            }
            Err(error) => {
                self.failed_revision = Some(result.revision);
                if self.wants_playback {
                    self.wants_playback = false;
                    self.error(format!("Cannot render this mix: {error}"));
                }
            }
        }
    }

    pub fn pause(&mut self) {
        self.cancel_playback_request();
        if let Some(player) = &mut self.player {
            player.pause();
        }
        self.previewing = false;
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.message("Paused.");
    }

    pub fn seek(&mut self, tick: u32) {
        self.master.reset_compressors();
        let was_playing = self.wants_playback;
        self.cursor_tick = tick;
        self.seek_pending = true;
        if was_playing {
            self.start_playback();
        }
    }

    pub fn preview(&mut self, sample_id: u32) {
        self.cancel_playback_request();
        let Some(sample) = self.library.sample_by_id(sample_id) else {
            return;
        };
        let name = sample.name.clone();
        match self.library.read_pcm(sample_id) {
            Ok(pcm) => match self.player.as_mut() {
                Some(player) => match pcm
                    .validate()
                    .and_then(|()| player.play(Arc::new(pcm), 0, None, None))
                {
                    Ok(()) => {
                        self.previewing = true;
                        self.preview_revision = self.preview_revision.wrapping_add(1);
                        self.message(format!("Listening: {name}"));
                    }
                    Err(error) => self.error(error),
                },
                None => self.error("Audio output is unavailable."),
            },
            Err(error) => self.error(format!("Cannot open {name}: {error}")),
        }
    }

    pub fn play_pcm(&mut self, pcm: Pcm, label: &str) {
        self.play_shared_pcm(Arc::new(pcm), label);
    }
    pub(super) fn play_shared_pcm(&mut self, pcm: Arc<Pcm>, label: &str) {
        self.cancel_playback_request();
        let result = self
            .player
            .as_mut()
            .ok_or("Audio output is unavailable.".to_string())
            .and_then(|p| pcm.validate().and_then(|()| p.play(pcm, 0, None, None)));
        match result {
            Ok(()) => {
                self.previewing = true;
                self.preview_revision = self.preview_revision.wrapping_add(1);
                self.message(label);
            }
            Err(error) => self.error(error),
        }
    }
    pub fn stop(&mut self) {
        self.master.reset_compressors();
        self.cancel_playback_request();
        if let Some(player) = &mut self.player {
            player.stop();
        }
        self.previewing = false;
        self.preview_revision = self.preview_revision.wrapping_add(1);
        self.cursor_tick = 0;
        if let Some(recorder) = self.recorder.take() {
            match recorder.finish() {
                Ok(pcm) => self.recorded_take = Some(Arc::new(pcm)),
                Err(error) => self.error(error),
            }
        } else {
            self.message("Stopped.");
        }
    }
    /// What a playback capture records of the session: the song, the
    /// transport, and the settings that shape playback.
    pub(super) fn capture_context(&self) -> serde_json::Value {
        let project = self.editor.project();
        let ticks = crate::engine::TICKS_PER_BAR;
        let position = self.player.as_ref().map(Player::position_frames);
        serde_json::json!({
            "song": {
                "title": project.title,
                "path": self.file,
                "unsaved_changes": self.unsaved,
                "bpm": project.bpm,
                "tracks": project.tracks.len(),
                "clips": project.clips.len(),
                "loop_region": project.loop_region,
                "revision": self.revision,
            },
            "transport": {
                "playing_mix": self.playing_mix(),
                "previewing": self.is_previewing(),
                "recording": self.is_recording(),
                "wants_playback": self.wants_playback,
                "seek_pending": self.seek_pending,
                "cursor_tick": self.cursor_tick,
                "bar": self.cursor_tick / ticks + 1,
                "tick_in_bar": self.cursor_tick % ticks,
                "position_frames": position,
                "frames_per_tick": self.playback_frames_per_tick,
                "mix_prepared": self.render.is_some() && !self.render_dirty,
            },
            "options": self.options,
            "master": project.master,
            "fx": project.fx,
        })
    }

    /// Save the last half minute that the device played, and the state of
    /// the song and the output, to `diagnostics/<local time>/` in the data
    /// folder. Shows the folder in the status line.
    pub fn save_playback_capture(
        &mut self,
        reason: &str,
    ) -> Result<(PathBuf, serde_json::Value), String> {
        let result = match &self.player {
            None => Err("Audio output is unavailable, so there is nothing to capture.".to_owned()),
            Some(player) => {
                player.save_capture(reason, &self.settings_root, self.capture_context())
            }
        };
        match result {
            Ok(saved) => {
                self.message(format!(
                    "Saved an audio capture to {}",
                    saved.folder.display()
                ));
                Ok((saved.folder, saved.state))
            }
            Err(error) => {
                self.error(format!("Cannot save an audio capture: {error}"));
                Err(error)
            }
        }
    }

    /// Keep the watcher's context current and report its captures.
    pub(super) fn poll_capture(&mut self) {
        if self.player.is_none() {
            return;
        }
        audio::flight::publish_context(std::time::Duration::from_secs(1), || {
            audio::flight::Context {
                data_root: self.settings_root.clone(),
                state: self.capture_context(),
            }
        });
        match audio::flight::take_automatic() {
            Some(Ok(folder)) => self.error(format!(
                "Playback sounded wrong, so an audio capture was saved to {}",
                folder.display()
            )),
            Some(Err(error)) => self.error(format!("Cannot save an audio capture: {error}")),
            None => {}
        }
    }
}
