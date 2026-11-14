//! What the window reports back to an agent: the state of the song and the
//! windows, the status line, the outcome of a request, and the lookup of the
//! controls and sounds a request names.
use super::*;

impl MixJamApp {
    pub(super) fn agent_target(
        &self,
        ctx: &egui::Context,
        target: &str,
        window: Option<String>,
        role: Option<String>,
        index: Option<usize>,
    ) -> Result<UiNode, String> {
        let node = find(
            &self.agent_nodes(ctx),
            target,
            window.as_deref(),
            role.as_deref(),
            index,
        )?;
        if node.enabled {
            Ok(node)
        } else {
            Err(match self.blocking_modal(ctx) {
                Some(modal) if node.window.as_deref() != Some(modal) => format!(
                    "\"{}\" is disabled while the {modal} modal is open.",
                    node.label
                ),
                _ => format!("\"{}\" is disabled.", node.label),
            })
        }
    }

    /// A sound by id, or by a name that only one sound has.
    pub(super) fn agent_sample(&self, sample: &Value) -> Result<u32, String> {
        let library = self.session.library();
        match sample {
            Value::Number(id) if id.fract() == 0.0 && (0.0..=f64::from(u32::MAX)).contains(id) => {
                let id = *id as u32;
                if library.sample_by_id(id).is_some() || self.session.missing_sample(id).is_some() {
                    Ok(id)
                } else {
                    Err(format!("No sound has the id {id}."))
                }
            }
            Value::Text(name) => {
                let named: Vec<&Sample> = library
                    .samples
                    .iter()
                    .filter(|sample| sample.name.eq_ignore_ascii_case(name))
                    .collect();
                match named[..] {
                    [sample] => Ok(sample.id),
                    [] => Err(format!(
                        "No sound is named \"{name}\". Use `samples search=` to find one."
                    )),
                    _ => Err(format!(
                        "{} sounds are named \"{name}\". Use one of these ids: {}.",
                        named.len(),
                        named
                            .iter()
                            .map(|sample| format!("{} ({})", sample.id, sample.group_name))
                            .collect::<Vec<_>>()
                            .join(", ")
                    )),
                }
            }
            _ => Err("Give the sound as an id or a name.".into()),
        }
    }

    fn open_windows(&self) -> Vec<String> {
        let eq = self.eq_track.map(|track| format!("Track {} EQ", track + 1));
        [
            (self.help.main, "Help"),
            (self.help.stretch, "Time Stretcher help"),
            (self.help.effects, "Effect Studio help"),
            (self.help.step_synth, "Step synth help"),
            (self.help.sample_edit, "Sample editor help"),
            (self.help.master, "Mastering help"),
            (self.options.is_some(), "Options"),
            (self.sample_import.details_open, "Import details"),
            (self.effects.open, "Effect Studio"),
            (self.board_shown(BottomModule::StepSynth), "Step synth"),
            (self.sample_edit.open, "Sample editor"),
            (self.stretch.open, "Time Stretcher"),
            (self.album_review.open, album_review::TITLE),
            (
                self.session.has_recorded_take() && !self.session.is_recording(),
                "Recorded take",
            ),
        ]
        .into_iter()
        .filter(|(open, _)| *open)
        .map(|(_, title)| title.to_owned())
        .chain(eq)
        .collect()
    }

    fn agent_status(&self) -> Status {
        Status {
            text: self.status.clone(),
            error: self.status_error,
        }
    }

    pub(super) fn agent_outcome(
        &self,
        ctx: &egui::Context,
        clips: Vec<u64>,
        control: Option<String>,
    ) -> Outcome {
        Outcome {
            status: self.agent_status(),
            modal: self.blocking_modal(ctx),
            windows: self.open_windows(),
            closing: self.files.allow_close,
            clips,
            control: control
                .and_then(|id| self.agent_nodes(ctx).into_iter().find(|node| node.id == id)),
        }
    }

    pub(super) fn agent_state(&self, ctx: &egui::Context) -> StateView {
        let mut song = wire::song_view(&self.session);
        if let Some(lanes) = self.score.rect {
            for (view, clip) in song.clips.iter_mut().zip(&self.session.project().clips) {
                let rect = self.clip_rect(clip, lanes);
                view.rect = (rect != Rect::NOTHING).then(|| rect_view(rect));
            }
        }
        StateView {
            version: mixjam_techno::application::APP_VERSION,
            song,
            options: self.session.options().clone(),
            options_revision: self.session.options_revision(),
            options_error: self.session.options_error().map(str::to_owned),
            audio_apply_blocked_reason: self
                .session
                .audio_options_blocked_reason()
                .map(str::to_owned),
            playback_buffer: self.session.buffer_status(),
            transport: wire::transport(&self.session),
            status: self.agent_status(),
            modal: self.blocking_modal(ctx),
            path_prompt: self.files.path_prompt.as_ref().map(|prompt| {
                let spec = self.path_spec(&prompt.request);
                PathPromptView {
                    title: spec.title,
                    kind: match spec.kind {
                        PathKind::OpenFile => "open_file",
                        PathKind::SaveFile => "save_file",
                        PathKind::Folder => "folder",
                    },
                    extensions: spec
                        .filter
                        .map(|(_, extensions)| extensions.to_vec())
                        .unwrap_or_default(),
                    suggested_name: spec.file_name,
                }
            }),
            windows: self.open_windows(),
            selection: Selection {
                sample: self.selection.sample,
                clips: self.selection.clips.iter().copied().collect(),
            },
            tools: Tools {
                erase: self.score_tools.erase,
                follow: self.score.follow_playhead,
                zoom: self.score.zoom,
                record_track: self.recording.track + 1,
            },
            busy: wire::busy(&self.session),
            arrangement: self
                .score
                .rect
                .zip(self.score.drop_rect)
                .map(|(lanes, visible)| ArrangementView {
                    visible: rect_view(visible),
                    left: lanes.left(),
                    top: lanes.top(),
                    bar_width: self.score.bar_width,
                    row_height: self.score.row_height,
                    tracks: self.score.tracks.iter().map(|track| track + 1).collect(),
                }),
        }
    }
}
