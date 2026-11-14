//! The modal and help windows: the blocking-modal precedence, the
//! unsaved-changes prompt, the Help windows of the application and its tools,
//! and the sample conflict resolver.
use super::*;

/// Which Help windows are open: the application's and each tool's.
#[derive(Default)]
pub(super) struct HelpWindows {
    pub(super) main: bool,
    pub(super) stretch: bool,
    pub(super) effects: bool,
    pub(super) step_synth: bool,
    pub(super) sample_edit: bool,
    pub(super) master: bool,
}

/// The sample conflict resolver: open, dismissed, and its last error.
#[derive(Default)]
pub(super) struct SampleConflicts {
    pub(super) open: bool,
    /// Escape hid the resolver without confirming; the status bar reopens it.
    pub(super) dismissed: bool,
    pub(super) error: Option<String>,
}

impl MixJamApp {
    /// Pending file choices suspend their parent confirmation, even while collapsed.
    pub(super) fn blocking_modal(&self, ctx: &egui::Context) -> Option<&'static str> {
        if self.files.path_prompt.is_some() {
            return if dialog::is_collapsed(ctx, "Choose path") {
                self.commands.as_ref().map(|_| "All commands")
            } else {
                Some("Choose path")
            };
        }
        if self.files.pending_action.is_some() && !dialog::is_collapsed(ctx, "Save your changes?") {
            return Some("Save your changes?");
        }
        if self.sample_conflicts_open() && !dialog::is_collapsed(ctx, "Sample conflicts") {
            return Some("Sample conflicts");
        }
        if self.sample_import.path.is_some() && !dialog::is_collapsed(ctx, "Import sound") {
            return Some("Import sound");
        }
        if self
            .options
            .as_ref()
            .is_some_and(|draft| draft.confirm_discard)
            && !dialog::is_collapsed(ctx, options::DISCARD)
        {
            return Some(options::DISCARD);
        }
        self.commands.as_ref().map(|_| "All commands")
    }

    pub(super) fn dialogs(&mut self, ctx: &egui::Context) {
        if !dialog::is_collapsed(ctx, "Save your changes?")
            && self.files.path_prompt.is_none()
            && std::mem::take(&mut self.files.resume_save_changes)
            && let Some(action) = self.files.pending_action.clone()
        {
            self.save_changes(action, ctx);
        }
        if let Some(action) = self.files.pending_action.clone()
            && self.files.path_prompt.is_none()
        {
            let response = dialog::modal(ctx, "Save your changes?", "Save your changes?", |ui| {
                ui.label(
                    if self.session.has_recorded_take() && self.session.unsaved() {
                        "The song and the recorded take are unsaved."
                    } else if self.session.has_recorded_take() {
                        "The recorded take is unsaved."
                    } else {
                        "The song has unsaved changes."
                    },
                );
                ui.horizontal(|ui| {
                    if dialog::primary(ui, "Save changes").clicked() {
                        self.save_changes(action.clone(), ctx);
                    }
                    if dialog::danger(ui, "Discard changes").clicked() {
                        self.run_action(action.clone(), ctx);
                    }
                    if controls::button(ui, "Cancel", controls::ButtonRole::Secondary).clicked() {
                        self.files.pending_action = None;
                    }
                });
            });
            if response.should_close() {
                self.files.pending_action = None;
                self.files.resume_save_changes = false;
            }
        }
        dialog::window("Help")
            .open(&mut self.help.main)
            .default_width(450.0)
            .default_height(420.0)
            .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, |ui| {
                egui::Grid::new("help_gestures")
                    .num_columns(2)
                    .spacing(Vec2::new(18.0, 8.0))
                    .show(ui, |ui| {
                        for (action, gesture) in [
                            (
                                "Place sound",
                                "Drag to a track, or select a sound and click a track. Up to 64 \
                                 tracks.",
                            ),
                            (
                                "Place by key",
                                "Tab to a sound and press Enter. It lands on the recording track \
                                 at the playhead.",
                            ),
                            (
                                "Cancel placing",
                                "An outline shows where a sound lands. Esc or a click on a clip \
                                 ends click placement.",
                            ),
                            ("Preview", "Double-click a sound or clip."),
                            ("Move / copy", "Drag a clip. Alt-drag copies."),
                            ("Select group", "Shift-click clips or drag an empty area."),
                            ("Fine snap", "Hold Ctrl for sixteenth-note positions."),
                            ("Delete", "Select clips, then press Delete."),
                            ("Select all", "Ctrl+A"),
                            (
                                "Duplicate",
                                "Ctrl+D copies the selected clips to the bars that follow.",
                            ),
                            (
                                "Clip menu",
                                "Right-click a clip or press Shift+F10 for exact gain, trim, \
                                 length, fades, pitch, Preview, Duplicate, Delete, and Resolve \
                                 manually for a missing sound.",
                            ),
                            (
                                "Clips by key",
                                "Tab to the lanes. Arrows choose a clip, Alt+arrows move it by a \
                                 bar or a track, Ctrl+Alt+arrows by a sixteenth. Enter previews.",
                            ),
                            (
                                "Seek / loop",
                                "Click a bar number to seek. Drag the ruler to loop.",
                            ),
                            ("Play / pause", "Space"),
                            ("Undo / redo", "Ctrl+Z / Ctrl+Shift+Z or Ctrl+Y"),
                            (
                                "Open / save",
                                "Ctrl+O / Ctrl+S. Ctrl+Shift+S saves as a new file. Open MIX, \
                                 MIXJAM, or JSON; save MIXJAM.",
                            ),
                            ("New song", "Ctrl+N"),
                            (
                                "Export",
                                "Stereo WAV of the mix. Export tracks… in the Mixer writes each \
                                 track and FX return to its own WAV.",
                            ),
                            (
                                "Audio capture",
                                "F12 saves the last 30 seconds that playback sent to the audio \
                                 device, with the song's state, to the diagnostics folder in the \
                                 data folder. Press it while something sounds wrong.",
                            ),
                            (
                                "Maximise",
                                "F hides the tool rail, the Sounds panel, the Inspector and \
                                 the lower panel; F again restores them.",
                            ),
                            ("Help", "F1"),
                        ] {
                            ui.add(egui::Label::new(action).wrap_mode(egui::TextWrapMode::Extend));
                            ui.add(egui::Label::new(gesture).wrap());
                            ui.end_row();
                        }
                    });
                ui.separator();
                ui.label(
                    "Mixer: gain, pan, L/R outputs, Mute, Solo, and master gain. Both outputs off \
                     leaves a track with no output.",
                );
                ui.label(
                    "M mutes a track. S solos it: while any track is soloed, only soloed tracks \
                     play, and a muted track stays silent. The strip says why a silent track is \
                     silent: Muted, No output, or Solo elsewhere.",
                );
                ui.label(
                    "Gain runs from -inf dB (silence) to +12 dB. 0 dB is unity. Negative dB \
                     reduces level; it does not invert polarity.",
                );
                ui.label(
                    "Click a gain value for exact dB entry, then Enter. Enter -inf for silence. \
                     Unity sits at 0 dB on the gain rail; Center resets pan on the pan rail. \
                     Master stays visible while tracks scroll.",
                );
                ui.label(
                    "The slashed-circle button reverses both channels' polarity without changing \
                     gain or pan. Its amber state means inverted. Polarity stays set at silence.",
                );
                ui.label(
                    "The tapered fader gives 72% of its travel to -24 through +12 dB. Arrow keys \
                     change 0.5 dB; hold Shift for 0.1 dB or fine dragging. Home silences and End \
                     sets +12 dB.",
                );
                ui.label(
                    "EQ opens a track's channel EQ: a low cut and a high cut at 12 dB per \
                     octave, a low shelf, two peaks with their Q, and a high \
                     shelf, drawn as a curve. It acts on the track's clips before its automation \
                     and fader. An EQ that is off keeps its settings and leaves the track as it \
                     was; Reset flattens every band. To tame harsh hi-hats, lower the high gain. \
                     The same window holds the track's compressor, stereo width, and ducker. \
                     The button lights when EQ, compression, or ducking is on, or width differs \
                     from 100%.",
                );
                ui.label(
                    "Bypass on the Master strip skips the master pedalboard for playback and \
                     export, as Mastering's Bypass all does.",
                );
                ui.label(
                    "Export tracks… in the Mixer's toolbar writes each track that plays, and the \
                     return of each FX bus that it feeds, to its own WAV file in a folder. A \
                     track's file holds the track alone after its EQ, automation, fader, pan, \
                     and outputs, without its sends; the FX returns hold the sends. The master \
                     pedalboard is left out, every file is as long as the mix export, and the \
                     files sum to the mix with the board bypassed when that sum does not clip.",
                );
                ui.label(
                    "Tab focuses controls. Arrow keys adjust sliders. Space toggles M, S, L, and \
                     R.",
                );
                ui.label(
                    "Shortcuts with Ctrl work from any control except a value that is being \
                     typed. Space plays when no control has focus, or when a dial, a fader, or \
                     the lanes have it.",
                );
                ui.separator();
                ui.label(egui::RichText::new("Time Stretcher").strong());
                stretch_help(ui);
                ui.separator();
                ui.label(egui::RichText::new("Effect Studio").strong());
                effects_help(ui);
                ui.separator();
                ui.label(egui::RichText::new("Step synth").strong());
                step_synth_help(ui);
                ui.separator();
                ui.label(egui::RichText::new("Sample Editor").strong());
                sample_editor_help(ui);
            });
        dialog::window("Time Stretcher help")
            .default_height(420.0)
            .open(&mut self.help.stretch)
            .default_width(450.0)
            .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, stretch_help);
        dialog::window("Effect Studio help")
            .default_height(420.0)
            .open(&mut self.help.effects)
            .default_width(450.0)
            .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, effects_help);
        dialog::window("Mastering help")
            .default_height(420.0)
            .open(&mut self.help.master)
            .default_width(450.0)
            .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, master_help);
        dialog::window("Step synth help")
            .default_height(420.0)
            .open(&mut self.help.step_synth)
            .default_width(450.0)
            .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, step_synth_help);
        dialog::window("Sample editor help")
            .default_height(420.0)
            .open(&mut self.help.sample_edit)
            .default_width(450.0)
            .max_height((ctx.viewport_rect().height() - 80.0).max(240.0))
            .constrain_to(ctx.viewport_rect().shrink(12.0))
            .show(ctx, sample_editor_help);
    }

    pub(super) fn sample_conflicts_open(&self) -> bool {
        !self.session.sample_conflicts().is_empty()
            && (self.conflicts.open
                || (!self.conflicts.dismissed
                    && self
                        .session
                        .sample_conflicts()
                        .iter()
                        .any(|conflict| !conflict.confirmed)))
    }

    pub(super) fn sample_conflicts_dialog(&mut self, ctx: &egui::Context) {
        if !self.sample_conflicts_open()
            || self.files.path_prompt.is_some()
            || (self.files.pending_action.is_some()
                && !dialog::is_collapsed(ctx, "Save your changes?"))
        {
            return;
        }
        self.conflicts.open = true;
        let conflicts = self.session.sample_conflicts().to_vec();
        let recording = self.session.is_recording();
        let mut choice = None;
        let mut confirm = false;
        let response = dialog::modal(ctx, "Sample conflicts", "Sample conflicts", |ui| {
            ui.set_width((ctx.viewport_rect().width() - 96.0).clamp(280.0, 700.0));
            ui.style_mut().wrap_mode = Some(egui::TextWrapMode::Wrap);
            ui.label(
                "Several files match a saved sample. The first match is selected until you choose \
                 another.",
            );
            ui.label("Each choice updates every instance of that source in this song.");
            ui.add_space(10.0);
            egui::ScrollArea::vertical()
                .id_salt("sample_conflict_choices")
                .max_height((ctx.viewport_rect().height() - 310.0).max(160.0))
                .show(ui, |ui| {
                    for (index, conflict) in conflicts.iter().enumerate() {
                        ui.push_id(index, |ui| {
                            if index > 0 {
                                ui.add_space(8.0);
                                ui.separator();
                                ui.add_space(8.0);
                            }
                            ui.add(
                                egui::Label::new(display_paths(&conflict.reference))
                                    .wrap()
                                    .selectable(true),
                            );
                            let instances = self
                                .session
                                .project()
                                .clips
                                .iter()
                                .filter(|clip| conflict.sample_ids.contains(&clip.sample_id))
                                .count();
                            ui.label(
                                egui::RichText::new(format!(
                                    "{instances} instances · {} matches",
                                    conflict.candidates.len()
                                ))
                                .color(roles().legend),
                            );
                            ui.add_enabled_ui(!recording, |ui| {
                                for (candidate_index, path) in
                                    conflict.candidates.iter().enumerate()
                                {
                                    let path = display_paths(&path.to_string_lossy());
                                    let selected = candidate_index == conflict.selected;
                                    let label = format!(
                                        "{}{}\n{path}",
                                        if selected {
                                            "Selected match "
                                        } else {
                                            "Use match "
                                        },
                                        candidate_index + 1
                                    );
                                    if controls::radio(ui, selected, &label).clicked() && !selected
                                    {
                                        choice = Some((index, candidate_index));
                                    }
                                }
                            });
                        });
                    }
                });
            ui.add_space(10.0);
            if recording {
                ui.colored_label(
                    roles().coral,
                    "Stop recording before changing or confirming sample choices.",
                );
                if controls::button(ui, "Stop recording", controls::ButtonRole::Secondary).clicked()
                {
                    self.stop();
                }
            }
            if let Some(error) = &self.conflicts.error {
                ui.colored_label(roles().coral, display_paths(error));
            }
            ui.horizontal(|ui| {
                if ui
                    .add_enabled_ui(!recording, |ui| dialog::primary(ui, "Keep choices"))
                    .inner
                    .clicked()
                {
                    confirm = true;
                }
                let playing = self.session.wants_playback();
                if ui
                    .add_enabled_ui(!recording, |ui| {
                        controls::button(
                            ui,
                            if playing { "Pause song" } else { "Play song" },
                            controls::ButtonRole::Secondary,
                        )
                    })
                    .inner
                    .clicked()
                {
                    if playing {
                        self.pause();
                    } else {
                        self.start_playback();
                    }
                }
            });
            ui.small(
                "Esc closes this window without confirming. Reopen from Sample conflicts in the \
                 status bar.",
            );
        });
        if let Some((conflict, candidate)) = choice {
            match self.session.resolve_sample_conflict(conflict, candidate) {
                Ok(()) => {
                    self.conflicts.error = None;
                    self.reconcile_selection();
                    self.message("Updated all instances of the selected source.");
                }
                Err(error) => self.conflicts.error = Some(error),
            }
        }
        if confirm && !recording {
            match self.session.confirm_sample_conflicts() {
                Ok(()) => {
                    self.conflicts.open = false;
                    self.conflicts.dismissed = false;
                    self.conflicts.error = None;
                    self.message("Sample choices retained.");
                }
                Err(error) => self.conflicts.error = Some(error),
            }
        } else if response.should_close() {
            // Escape or a click outside only hides the window. Confirmation is
            // an undoable project change, so it needs the explicit button.
            self.conflicts.open = false;
            self.conflicts.dismissed = true;
            self.conflicts.error = None;
            self.message(
                "Sample choices are not confirmed. Reopen from Sample conflicts in the status bar.",
            );
        }
    }
}

fn stretch_help(ui: &mut egui::Ui) {
    for text in [
        "Original BPM describes the recording. Target BPM sets the result to 120, 125, 130, 135, 140, 160, or 180 BPM.",
        "A lower target makes the sound longer; a higher target makes it shorter. Pitch preservation is approximate and can change transients or add audible artifacts. Listen before saving.",
        "Click Original BPM for exact entry. Drag the Original knob, use the mouse wheel, or focus it and press an arrow key. Hold Shift while dragging for finer changes. For Target BPM, click or drag to a notch, use the mouse wheel, or focus the slider and press an arrow key.",
        "Double-click Original to restore the selected source tempo. Double-click Target to reset to 140 BPM.",
        "Listen changes to Stop listening during preview. The status line reports processing, playback, errors, and a saved sound.",
        "Saving keeps the source and both tempos, so another version is one adjustment away. Esc closes the tool; it first closes an open list or cancels a value being typed.",
    ] {
        ui.add(egui::Label::new(text).wrap());
    }
}

fn master_help(ui: &mut egui::Ui) {
    for text in [
        "Mastering processes the whole mix after the tracks are summed. Choose its tab in the lower panel, or its button in the tool rail. The signal flows from left to right: the mixer's output enters the Input unit, passes each pedal that is on, and leaves through the Output unit. Playback and export both use it, so what you hear is what you export.",
        "Preset loads a factory board. Subtle, Revive, and Loud suit any finished mix, from a light polish to a loud master; the genre presets follow them. A preset sets every pedal and the order, turns the Booster pedals off, and ends at the Limiter with its ceiling at -1 dBTP. It keeps your Trim: if you turned the mix down because it was too loud, it stays down. One Undo restores the board before it. The selector reads Custom once you change a pedal. Songs save the board, not the preset's name.",
        "Input: Trim sets the level into the board. The VU meter reads the trimmed mix; 0 VU is -18 dBFS, where the mastering pedals work best. The peak lamps light at -1 dBFS.",
        "Output: momentary and short-term loudness follow the last 400 ms and 3 s; integrated loudness and true peak cover the playback so far. The green band marks -14 LUFS and the red line -1 dBTP. The OVER lamp lights when the true peak passes -1 dBTP and stays lit until you click it. The meters measure during playback.",
        "The Booster pedals reproduce the master effects of the original D3, D4, and T3 products: a ten-band EQ, a compressor, and stereo wide. Each playback starts with settled compressor controls. Their EQ bands show decibels; Drive, Speed, Gain, and Width use the original's 0 to 1000 scale.",
        "The mastering pedals are new designs: Soft clip rounds peaks off; Tube adds warm, mostly even harmonics; Trim EQ cleans up rumble, mud, and harshness; Bus compressor glues the mix; Maximizer adds loudness under a fixed ceiling; Lift EQ adds weight and air; Tape adds saturation with a low-end bump; Stereo imaging sets the width and keeps the lows mono; Multiband holds the peaks of the lows, mids, and highs apart and lifts the sound under them; Limiter keeps the true peak under its ceiling. The Bus compressor and Limiter show their gain reduction on lamps.",
        "Drag a pedal by its name to move it along the board; near either end the row scrolls. The arrow buttons move a pedal one place. The Limiter cannot move: it always runs last, just before the Output unit, so no pedal can raise the peaks it holds. The strip below the board shows every pedal in order; click one to bring it into view. In a short window, scroll the board to reach the strip. Each move is one Undo step.",
        "Bypass all, beside Preset, skips the whole board, Trim included, for playback and export, so you can export the mix and master it elsewhere. The pedals keep their settings and switches, and the board dims. Switching it during playback crossfades over 30 ms. Presets leave it as it is. The Mixer's Master strip has the same switch.",
        "Each pedal has an on/off switch at its foot and a lamp beside its name. A pedal that is off keeps its settings and passes the signal on. When every pedal is off and Trim is at 0 dB, the mix plays exactly as the tracks sum. Some pedals delay the sound by a few milliseconds; the board's state beside Preset shows the total, and exports remove it.",
        "Songs open with every pedal off unless their file switched one on. Importing a legacy D3, D4, or T3 .mix song applies its saved Booster settings; importing a MixJam v7 project applies its master bus.",
        "Drag a knob, turn the mouse wheel, or focus it and press an arrow key. Hold Shift for fine adjustments. Click a value for exact entry. Double-click a knob to restore its default. Changes apply while the song plays, and Undo restores them.",
    ] {
        ui.add(egui::Label::new(text).wrap());
    }
}

fn effects_help(ui: &mut egui::Ui) {
    for text in [
        "Select a palette sound or a clip, then open Effect Studio. Listen previews the processed sound. Save sound writes a new WAV, adds it to Effect, and selects it.",
        "Filter, Echo, and Robotize each have an on/off switch beside their name. Turning a section off discards its settings. Turning it on restores that section's defaults.",
        "Drag a knob, turn the mouse wheel, or focus it and press an arrow key. Hold Shift for fine adjustments. Click a value for exact entry. Double-click a knob to restore its default.",
        "Cutoff and Color use logarithmic ranges. Echo delay is measured in beats at the source tempo. Echo tails stop at the sample boundary.",
        "Dirty distortion, Noise distortion, and Output volume are always available. Reset restores all defaults. The effects are the application's own algorithms.",
        "The upper-left triangle collapses the panel without changing its settings. Esc closes the tool after a focused value, list, or window above it has handled the key.",
    ] {
        ui.add(egui::Label::new(text).wrap());
    }
}

fn step_synth_help(ui: &mut egui::Ui) {
    for text in [
        "Choose a WAV or PXD source file, or use the selected palette sound or clip. Listen sequence previews the active bars. Save sequence writes a WAV to Step synth at the sequence tempo.",
        "Each of four bars has sixteen steps. Click a cell to set its pitch, replace that step's note, or click the same note again to clear it. Scroll the pitch grid to reach all notes from +12 to -12 semitones. A 0 keeps the source pitch.",
        "View bar changes the bar you edit. Active switches choose which bars render, in ascending order. Clear bar clears only the viewed bar. At least one bar must be active to render or save a pattern.",
        "1/16 cut ends a trigger after one sixteenth. Without it, the source continues until the next trigger or sequence end. Loop source repeats audio during that interval. The tempo choices are 120, 125, 130, 135, 140, 160, and 180 BPM.",
        "Sequence effects replaces the note grid with the effects form. Note grid returns to editing. Save pattern and Load pattern keep the editable four-bar draft in JSON; they do not replace the source file or viewed bar.",
        "The Step synth button in the tool rail folds the lower panel without changing the draft. Choose the Step synth tab to open it again. Tab reaches each control and the note grid. In the grid, arrow keys move between cells, Home and End jump to the first and last step, and Space or Enter sets or clears the note; the focused cell scrolls into view. Esc first cancels an active value or list, or goes to a window above the board.",
        "Resampling, source looping, and effects are the application's own algorithms.",
    ] {
        ui.add(egui::Label::new(text).wrap());
    }
}

fn sample_editor_help(ui: &mut egui::Ui) {
    for text in [
        "Select a palette sound or a clip, then open Sample Editor. Listen edit previews the result. Save edited sound writes a new WAV, adds it to Wave, and leaves the source unchanged.",
        "Start frame is the first frame that is kept, counted from zero. End frame is the first frame that is left out; set it to the frame count to keep the last frame. A frame is one sample of audio.",
        "Click a value for exact entry, then press Enter. The + and - buttons change one frame, or 100 with Shift.",
        "Pitch runs from -24 to +24 semitones and changes speed and length together: +12 is an octave up and half as long, -12 an octave down and twice as long. Its buttons change 0.1 semitone, or one with Shift.",
        "The status line shows the selected frames and the length after pitch, or why editing is unavailable. A missing sound plays silence; resolve it in Import details.",
        "The upper-left triangle collapses the tool and keeps the edit. Esc closes the tool after a value being typed or a window above it has handled the key.",
    ] {
        ui.add(egui::Label::new(text).wrap());
    }
}
