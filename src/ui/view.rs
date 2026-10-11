use super::*;

/// Format Windows filesystem paths for labels only. Stored paths and messages
/// retain their original spelling for file operations and diagnostics.
pub(super) fn display_paths(text: &str) -> String {
    let mut display = String::with_capacity(text.len());
    let mut remaining = text;
    while let Some(start) = remaining.find(r"\\?\") {
        display.push_str(&remaining[..start]);
        let tail = &remaining[start + 4..];
        if let Some(unc) = tail.strip_prefix(r"UNC\") {
            display.push_str(r"\\");
            remaining = unc;
        } else {
            let bytes = tail.as_bytes();
            if !(bytes.len() >= 3
                && bytes[0].is_ascii_alphabetic()
                && bytes[1] == b':'
                && bytes[2] == b'\\')
            {
                // Device and volume identifiers have no equivalent drive path.
                display.push_str(r"\\?\");
            }
            remaining = tail;
        }
    }
    display.push_str(remaining);
    display
}

// Shared visual tokens. Application state and rules live outside this module.
pub(super) const CANVAS: Color32 = Color32::from_rgb(20, 23, 27);
pub(super) const PANEL: Color32 = Color32::from_rgb(28, 32, 37);
pub(super) const TEXT: Color32 = Color32::from_rgb(233, 237, 238);
pub(super) const ACCENT: Color32 = Color32::from_rgb(184, 227, 126);
pub(super) const CORAL: Color32 = Color32::from_rgb(255, 130, 130);
const MUTED: Color32 = Color32::from_rgb(164, 174, 182);
const LINE: Color32 = Color32::from_rgb(58, 65, 72);
const RAISED: Color32 = Color32::from_rgb(39, 45, 51);
const HOVER: Color32 = Color32::from_rgb(51, 59, 66);
const GROUPS: [&str; 10] = [
    "Loop",
    "Drum",
    "Bass",
    "Sequence",
    "Wave",
    "Voice",
    "Effect",
    "Xtra",
    "Sphere",
    "Step synth",
];

fn heading(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(text)
        .font(FontId::new(
            size,
            egui::FontFamily::Name("Space Grotesk Bold".into()),
        ))
        .color(TEXT)
}

impl MixJamApp {
    pub(super) fn header(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::top("header")
            .exact_height(66.0)
            .frame(
                egui::Frame::new()
                    .fill(CANVAS)
                    .inner_margin(egui::Margin::symmetric(22, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    ui.label(heading("MixJam", 27.0));
                    ui.label(egui::RichText::new("Techno").size(14.0).color(MUTED));
                    ui.add_space(24.0);
                    ui.separator();
                    ui.add_space(18.0);
                    let title = if self.session.unsaved() {
                        format!("{} *", self.session.project().title)
                    } else {
                        self.session.project().title.clone()
                    };
                    let title_width = (ui.available_width() - 290.0).max(40.0);
                    ui.add_sized(
                        [title_width, 30.0],
                        egui::Label::new(heading(title, 15.0)).truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if action_button(ui, "Export WAV", true).clicked() {
                            self.export();
                        }
                        if action_button(ui, "Help", false).clicked() {
                            self.show_help = !self.show_help;
                        }
                        ui.add_space(8.0);
                        if action_button(ui, "Redo", false).clicked() {
                            self.history_step(true);
                        }
                        if action_button(ui, "Undo", false).clicked() {
                            self.history_step(false);
                        }
                    });
                });
            });
    }

    pub(super) fn side_rail(&mut self, ctx: &egui::Context) {
        let icons = self
            .rail_icons
            .get_or_insert_with(|| RailIcons::load(ctx).map(Arc::new))
            .clone();
        let icons = match icons {
            Ok(icons) => icons,
            Err(error) => {
                self.error(error);
                return;
            }
        };
        egui::SidePanel::right("tools")
            .exact_width(64.0)
            .resizable(false)
            .frame(egui::Frame::new().fill(CANVAS).inner_margin(6))
            .show(ctx, |ui| {
                ui.add_space(4.0);
                ui.vertical_centered(|ui| {
                    rail_heading(ui, &icons, RailIcon::Mix);
                    ui.add_space(4.0);
                    if rail_button(ui, &icons, RailIcon::Open, false).clicked() {
                        self.request_action(PendingAction::Open, ctx);
                    }
                    if rail_button(ui, &icons, RailIcon::Save, false).clicked() {
                        self.save(false);
                    }
                    if rail_button(ui, &icons, RailIcon::SaveAs, false).clicked() {
                        self.save(true);
                    }
                    if rail_button(ui, &icons, RailIcon::Export, false).clicked() {
                        self.export();
                    }
                    if rail_button(ui, &icons, RailIcon::Import, false).clicked() {
                        self.import_wav();
                    }
                    if rail_button(ui, &icons, RailIcon::Libraries, false).clicked() {
                        self.show_libraries = true;
                    }
                    ui.add_space(10.0);
                    if rail_button(ui, &icons, RailIcon::New, false).clicked() {
                        self.request_action(PendingAction::New, ctx);
                    }
                    ui.add_space(10.0);
                    if rail_button(ui, &icons, RailIcon::Mixer, self.show_mixer).clicked() {
                        self.show_mixer = !self.show_mixer;
                    }
                    if rail_button(ui, &icons, RailIcon::Erase, self.erase).clicked() {
                        self.erase = !self.erase;
                    }
                    if rail_button(ui, &icons, RailIcon::Delete, false).clicked() {
                        self.delete_selection();
                    }
                    ui.add_space(8.0);
                    if rail_button(ui, &icons, RailIcon::Effects, false).clicked() {
                        self.show_effects = true;
                    }
                    if rail_button(ui, &icons, RailIcon::StepSynth, false).clicked() {
                        self.show_step_synth = true;
                    }
                    if rail_button(ui, &icons, RailIcon::Edit, false).clicked() {
                        self.show_sample_edit = true;
                        self.edit_start = 0;
                        self.edit_end = 0;
                    }
                    if rail_button(ui, &icons, RailIcon::Stretch, false).clicked() {
                        self.show_stretch = true;
                    }
                    ui.add_space(8.0);
                    rail_heading(ui, &icons, RailIcon::Record);
                    let occupied = occupied_tracks(self.session.project());
                    recording_destination(ui, &mut self.armed_track, &occupied);
                });
            });
    }

    pub(super) fn bottom(&mut self, ctx: &egui::Context) {
        egui::TopBottomPanel::bottom("status")
            .exact_height(30.0)
            .frame(
                egui::Frame::new()
                    .fill(CANVAS)
                    .inner_margin(egui::Margin::symmetric(18, 6)),
            )
            .show(ctx, |ui| {
                ui.horizontal(|ui| {
                    if (!self.session.project().import_notices.is_empty()
                        || !self.session.project().source_notices.is_empty())
                        && ui.small_button("Import details").clicked()
                    {
                        self.show_import_details = true;
                    }
                    let color = if self.status_error { CORAL } else { MUTED };
                    let status = display_paths(&self.status);
                    ui.add(
                        egui::Label::new(egui::RichText::new(&status).size(12.0).color(color))
                            .truncate(),
                    )
                    .on_hover_text(&status);
                });
            });
        let height = (ctx.content_rect().height() * 0.28).clamp(220.0, 290.0);
        // Reserve the transport and a usable arrangement, including its inspector.
        let max_height = (ctx.available_rect().height() - 68.0 - 220.0).max(180.0);
        let sounds = egui::TopBottomPanel::bottom("sounds")
            .default_height(height)
            .height_range(180.0..=max_height)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());
                if self.show_mixer {
                    self.mixer(ui);
                } else {
                    self.palette(ui);
                }
            });
        let grip = Rect::from_center_size(
            Pos2::new(
                sounds.response.rect.center().x,
                sounds.response.rect.top() + 2.0,
            ),
            Vec2::new(48.0, 3.0),
        );
        ctx.layer_painter(egui::LayerId::background())
            .rect_filled(grip, 2.0, MUTED);
        egui::TopBottomPanel::top("transport")
            .exact_height(68.0)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(18, 10)),
            )
            .show(ctx, |ui| self.transport(ui));
    }

    pub(super) fn transport(&mut self, ui: &mut egui::Ui) {
        ui.horizontal_centered(|ui| {
            transport_position(ui, self.session.cursor_tick());
            ui.add_space(12.0);
            if transport_button(
                ui,
                TransportIcon::Loop,
                self.session.project().loop_region.is_some(),
                "Loop selected bars",
            )
            .clicked()
            {
                let end = self.session.project().end_tick().max(TICKS_PER_BAR);
                let region = if self.session.project().loop_region.is_some() {
                    None
                } else {
                    Some([0, end])
                };
                let result = self.session.edit(EditCommand::SetLoop(region));
                self.apply(result);
            }
            if transport_button(ui, TransportIcon::Start, false, "Go to start").clicked() {
                self.seek(0);
            }
            if transport_button(ui, TransportIcon::Back, false, "Back one bar").clicked() {
                self.seek(self.session.cursor_tick().saturating_sub(TICKS_PER_BAR));
            }
            if transport_button(ui, TransportIcon::Stop, false, "Stop").clicked() {
                self.stop();
            }
            let play = transport_button(
                ui,
                if self.session.wants_playback() {
                    TransportIcon::Pause
                } else {
                    TransportIcon::Play
                },
                self.session.wants_playback(),
                "Play / pause (Space)",
            );
            if self.capture_transport_focus {
                play.request_focus();
                self.capture_focus_ready = play.has_focus();
                if !self.capture_focus_ready {
                    ui.ctx().send_viewport_cmd(egui::ViewportCommand::Focus);
                }
            }
            if play.clicked() {
                if self.session.wants_playback() {
                    self.pause();
                } else {
                    self.start_playback();
                }
            }
            if transport_button(ui, TransportIcon::Forward, false, "Forward one bar").clicked() {
                self.seek(self.session.cursor_tick().saturating_add(TICKS_PER_BAR));
            }
            if transport_button(ui, TransportIcon::End, false, "Go to end").clicked() {
                self.seek(self.session.project().end_tick());
            }
            if transport_button(
                ui,
                TransportIcon::Record,
                self.session.is_recording(),
                "Record into the armed track",
            )
            .clicked()
            {
                if self.session.is_recording() {
                    self.stop();
                } else {
                    self.start_recording();
                }
            }
            ui.add_space(12.0);
            ui.label(egui::RichText::new("BPM").color(MUTED));
            let mut bpm = self.session.project().bpm;
            if bpm_picker(ui, "project_bpm", &mut bpm).changed() {
                let result = self.session.edit(EditCommand::SetTempo(bpm));
                self.apply(result);
            }
            ui.add_space(10.0);
            ui.label("Zoom");
            ui.add(
                egui::Slider::new(&mut self.zoom, 0.55..=2.0)
                    .show_value(false)
                    .clamping(egui::SliderClamping::Always),
            );
            if ui
                .toggle_value(&mut self.follow_playhead, "Follow")
                .on_hover_text("Keep the playhead in view during playback and seeking.")
                .changed()
            {
                self.last_follow_position = None;
            }
        });
    }

    pub(super) fn palette(&mut self, ui: &mut egui::Ui) {
        let samples = self.palette_indices();
        let group_names: BTreeSet<&str> = self
            .session
            .library()
            .samples
            .iter()
            .map(|s| s.group_name.as_str())
            .collect();
        let groups: BTreeMap<String, &str> = group_names
            .into_iter()
            .map(|name| (name.to_lowercase(), name))
            .collect();
        ui.horizontal(|ui| {
            ui.label(heading("Sounds", 18.0));
            if action_button(ui, "Mixer", false).clicked() {
                self.show_mixer = true;
            }
            ui.add_space(12.0);
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Find a sound")
                    .desired_width(170.0),
            );
            egui::ComboBox::from_id_salt("palette_product")
                .selected_text(self.palette_product.map_or("All libraries", Product::label))
                .width(145.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.palette_product, None, "All libraries");
                    for product in Product::ALL {
                        ui.selectable_value(
                            &mut self.palette_product,
                            Some(product),
                            product.label(),
                        );
                    }
                });
            let group_label = self.group.as_deref().map_or("All sounds", |g| {
                if g.is_empty() { "Uncategorized" } else { g }
            });
            egui::ComboBox::from_id_salt("palette_category")
                .selected_text(group_label)
                .width(105.0)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.group, None, "All sounds");
                    for name in groups.values() {
                        ui.selectable_value(
                            &mut self.group,
                            Some((*name).to_owned()),
                            if name.is_empty() {
                                "Uncategorized"
                            } else {
                                name
                            },
                        );
                    }
                });
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} sounds", samples.len()))
                        .size(12.0)
                        .color(MUTED),
                );
            });
        });
        ui.add_space(7.0);
        egui::ScrollArea::horizontal()
            .id_salt("sound_categories")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    if category_button(ui, "All", self.group.is_none()).clicked() {
                        self.group = None;
                    }
                    for name in GROUPS {
                        if category_button(ui, name, self.group.as_deref() == Some(name)).clicked()
                        {
                            self.group = Some(name.into());
                        }
                    }
                });
            });
        ui.add_space(10.0);
        if samples.is_empty() {
            ui.add_space(12.0);
            ui.label(heading(
                if self.session.library().samples.is_empty() {
                    "Your next mix starts with a sound."
                } else {
                    "No matching sounds"
                },
                17.0,
            ));
            ui.label(
                egui::RichText::new(if self.session.library().samples.is_empty() {
                    "Choose a WAV folder, then drag sounds into the arrangement."
                } else {
                    "Try another category or library, or clear your search."
                })
                .color(MUTED),
            );
            if self.session.library().samples.is_empty() {
                if action_button(ui, "Choose WAV folder", true).clicked() {
                    self.show_libraries = true;
                }
            } else if action_button(ui, "Clear filters", false).clicked() {
                self.search.clear();
                self.group = None;
                self.palette_product = None;
            }
        }
        let count = ((ui.available_width() + 8.0) / 156.0).floor().max(1.0) as usize;
        egui::ScrollArea::vertical()
            .id_salt("sample_palette")
            .max_height(ui.available_height())
            .show_rows(ui, 46.0, samples.len().div_ceil(count), |ui, rows| {
                egui::Grid::new("sound_grid")
                    .spacing(Vec2::new(8.0, 6.0))
                    .show(ui, |ui| {
                        for row in rows {
                            for &sample_index in
                                &samples[row * count..((row + 1) * count).min(samples.len())]
                            {
                                let sample = self.session.library().samples[sample_index].clone();
                                let source = self
                                    .sample_product(sample.id)
                                    .map_or("User WAV", Product::label);
                                let response = ui
                                    .push_id(sample.id, |ui| {
                                        sample_button(
                                            ui,
                                            &sample,
                                            self.selected_sample == Some(sample.id),
                                        )
                                    })
                                    .inner
                                    .on_hover_text(format!("{} / {}", source, sample.group_name));
                                if response.clicked() {
                                    self.selected_sample = Some(sample.id);
                                }
                                if response.double_clicked() {
                                    self.preview(sample.id);
                                }
                                if response.drag_started() {
                                    self.selected_sample = Some(sample.id);
                                    self.drag = Some(DragItem::Sample(sample.id));
                                }
                            }
                            ui.end_row();
                        }
                    });
            });
    }

    pub(super) fn mixer(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(heading("Mixer", 18.0));
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if action_button(ui, "Sounds", false).clicked() {
                    self.show_mixer = false;
                }
            });
        });
        let mut tracks = self.session.project().tracks.clone();
        let occupied = occupied_tracks(self.session.project());
        let mut gain = self.session.project().master_gain;
        let mut change = false;
        let fader_height = (ui.available_height() - 120.0).max(32.0);
        ui.scope(|ui| {
            ui.visuals_mut().override_text_color = Some(TEXT);
            ui.visuals_mut().selection.bg_fill = ACCENT;
            egui::ScrollArea::horizontal()
                .id_salt("mixer_scroll")
                .show(ui, |ui| {
                    ui.horizontal_top(|ui| {
                        for &index in &occupied {
                            let track = &mut tracks[index];
                            ui.allocate_ui_with_layout(
                                Vec2::new(76.0, fader_height + 84.0),
                                egui::Layout::top_down(egui::Align::Center),
                                |ui| {
                                    ui.spacing_mut().slider_width = fader_height;
                                    ui.horizontal(|ui| {
                                        change |= route_button(ui, "L", &mut track.left_enabled)
                                            .changed();
                                        change |= route_button(ui, "R", &mut track.right_enabled)
                                            .changed();
                                    });
                                    change |= ui
                                        .add_sized(
                                            [25.0, fader_height],
                                            egui::Slider::new(&mut track.gain, -2.0..=4.0)
                                                .vertical()
                                                .show_value(false),
                                        )
                                        .changed();
                                    ui.label(
                                        egui::RichText::new(format!("{}", index + 1))
                                            .strong()
                                            .color(TEXT),
                                    );
                                    ui.spacing_mut().slider_width = 52.0;
                                    change |= pan_control(ui, index, &mut track.pan).changed();
                                    ui.small("L  Pan  R");
                                },
                            );
                        }
                        ui.add_space(18.0);
                        ui.allocate_ui_with_layout(
                            Vec2::new(66.0, fader_height + 48.0),
                            egui::Layout::top_down(egui::Align::Center),
                            |ui| {
                                ui.spacing_mut().slider_width = fader_height;
                                ui.label(egui::RichText::new("Master").color(ACCENT));
                                change |= ui
                                    .add_sized(
                                        [25.0, fader_height],
                                        egui::Slider::new(&mut gain, 0.0..=2.0)
                                            .vertical()
                                            .show_value(false),
                                    )
                                    .changed();
                                ui.label(format!("{:.0}%", gain * 100.0));
                            },
                        );
                    });
                });
        });
        if change {
            let result = self.session.edit(EditCommand::SetMixer {
                tracks,
                master_gain: gain,
            });
            self.apply(result);
        }
    }

    pub(super) fn selected_inspector(&mut self, ui: &mut egui::Ui) {
        let Some(id) = self.selected_clip else { return };
        let Some(clip) = self
            .session
            .project()
            .clips
            .iter()
            .find(|c| c.id == id)
            .cloned()
        else {
            self.selected_clip = None;
            return;
        };
        let Some(sample) = self.session.library().sample_by_id(clip.sample_id) else {
            return;
        };
        let name = sample.name.clone();
        let max_frames = sample.frame_count;
        let mut gain = clip.gain;
        let mut offset = clip.offset_frames;
        let mut length = clip.length_ticks;
        ui.horizontal_wrapped(|ui| {
            ui.label(egui::RichText::new(name).color(ACCENT).strong());
            ui.add_space(12.0);
            ui.label("Gain");
            let mut change = ui
                .add(egui::DragValue::new(&mut gain).range(0.0..=2.0).speed(0.01))
                .changed();
            ui.label("Trim start (frames)");
            change |= ui
                .add(
                    egui::DragValue::new(&mut offset)
                        .range(0..=max_frames.saturating_sub(1))
                        .speed(100),
                )
                .changed();
            ui.label("Length (bars)");
            let mut bars = length as f64 / TICKS_PER_BAR as f64;
            if ui
                .add(
                    egui::DragValue::new(&mut bars)
                        .range(1.0 / 16.0..=256.0)
                        .speed(1.0 / 16.0),
                )
                .changed()
            {
                length = (bars * TICKS_PER_BAR as f64).round().max(1.0) as u32;
                change = true;
            }
            if change {
                let result = self.session.edit(EditCommand::SetClip {
                    id,
                    gain,
                    offset_frames: offset,
                    length_ticks: length,
                });
                self.apply(result);
            }
        });
    }

    pub(super) fn timeline(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label(heading("Arrangement", 18.0));
            ui.add_space(10.0);
            ui.label(
                egui::RichText::new(format!(
                    "{} tracks  /  {} clips",
                    occupied_tracks(self.session.project()).len(),
                    self.session.project().clips.len()
                ))
                .size(12.0)
                .color(MUTED),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("Drag sounds here  ·  Ctrl: fine snap")
                        .size(12.0)
                        .color(MUTED),
                );
            });
        });
        ui.add_space(12.0);
        self.timeline_rect = None;
        self.timeline_drop_rect = None;
        self.timeline_ruler_rect = None;
        let occupied = occupied_tracks(self.session.project());
        let occupied_count = occupied.len();
        self.timeline_add_track =
            if self.armed_track < TRACK_COUNT && !occupied.contains(&self.armed_track) {
                Some(self.armed_track)
            } else {
                (0..TRACK_COUNT).find(|index| !occupied.contains(index))
            };
        self.timeline_tracks = occupied;
        self.timeline_tracks.extend(self.timeline_add_track);
        self.timeline_row_by_track.fill(None);
        for (row, &track) in self.timeline_tracks.iter().enumerate() {
            self.timeline_row_by_track[track] = Some(row);
        }
        let visible_tracks = self.timeline_tracks.clone();
        let add_track = self.timeline_add_track;
        let row_count = visible_tracks.len();
        let row_height =
            ((ui.available_height() - 69.0) / row_count.clamp(1, 16) as f32).clamp(34.0, 48.0);
        let bar_width = 62.0 * self.zoom;
        self.timeline_row_height = row_height;
        self.timeline_bar_width = bar_width;
        let track_height = row_height * row_count as f32;
        let cursor_tick = self.session.cursor_tick();
        let bars =
            (self.session.project().end_tick().max(cursor_tick) / TICKS_PER_BAR + 16).max(64);
        let width = bars as f32 * bar_width;
        let label_width = 74.0;
        let lane_inset = label_width + ui.spacing().item_spacing.x;
        let inspector_height = if self.selected_clip.is_some() {
            34.0
        } else {
            0.0
        };
        let viewport_height = (ui.available_height() - 26.0 - inspector_height - 8.0).max(80.0);
        let (ruler_strip, _) =
            ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
        let body_left = ruler_strip.left();
        let mut route_change = None;
        let mut painted_offset_x = 0.0;
        let mut scroll_area = egui::ScrollArea::both()
            .id_salt("arrangement_scroll")
            .scroll_source(egui::scroll_area::ScrollSource {
                drag: false,
                ..Default::default()
            })
            .auto_shrink([false, false])
            .max_height(viewport_height);
        // Only a position or geometry change can move a paused viewport. Manual
        // scrolling stays put until the playhead moves or Follow is enabled again.
        let visible_width =
            (ui.available_width() - lane_inset - ui.spacing().scroll.allocated_width()).max(1.0);
        let follow_position = (cursor_tick, bar_width, visible_width);
        if self.follow_playhead && self.last_follow_position != Some(follow_position) {
            let id = ui.make_persistent_id(egui::Id::new("arrangement_scroll"));
            let offset = egui::scroll_area::State::load(ui.ctx(), id)
                .unwrap_or_default()
                .offset
                .x;
            let playhead = cursor_tick as f32 / TICKS_PER_BAR as f32 * bar_width;
            if let Some(offset) = follow_offset(playhead, offset, visible_width, width) {
                scroll_area = scroll_area.horizontal_scroll_offset(offset);
            }
        }
        self.last_follow_position = Some(follow_position);
        if self.capture_last_track {
            scroll_area = scroll_area.vertical_scroll_offset(track_height);
            self.capture_last_track = false;
        }
        let scroll = scroll_area.show_viewport(ui, |ui, viewport| {
            painted_offset_x = viewport.left();
            let (whole, _) =
                ui.allocate_exact_size(Vec2::new(width + lane_inset, track_height), Sense::hover());
            let rect = Rect::from_min_size(
                whole.min + Vec2::new(lane_inset, 0.0),
                Vec2::new(width, track_height),
            );
            let canvas_clip = ui.clip_rect().intersect(Rect::from_min_max(
                Pos2::new(body_left + lane_inset, whole.top()),
                Pos2::new(ui.clip_rect().right(), whole.bottom()),
            ));
            ui.scope_builder(
                egui::UiBuilder::new().max_rect(rect).id_salt("lanes"),
                |ui| {
                    ui.set_clip_rect(canvas_clip);
                    let response =
                        ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag());
                    self.timeline_rect = Some(rect);
                    if response.hovered()
                        && let Some(pos) = ui.input(|i| i.pointer.latest_pos())
                        && let Some(clip) = self
                            .session
                            .project()
                            .clips
                            .iter()
                            .rev()
                            .find(|clip| self.clip_rect(clip, rect).contains(pos))
                        && let Some(sample) = self.session.library().sample_by_id(clip.sample_id)
                    {
                        response.clone().on_hover_text(&sample.name);
                    }
                    self.timeline_drop_rect = Some(rect.intersect(ui.clip_rect()));
                    let painter = ui.painter_at(rect).with_clip_rect(canvas_clip);
                    painter.rect_filled(rect, 0.0, CANVAS);
                    for track in 0..occupied_count {
                        let row = Rect::from_min_size(
                            Pos2::new(rect.left(), rect.top() + track as f32 * row_height),
                            Vec2::new(width, row_height),
                        );
                        painter.rect_filled(
                            row,
                            0.0,
                            if track % 2 == 0 {
                                CANVAS
                            } else {
                                Color32::from_rgb(24, 28, 32)
                            },
                        );
                        painter.line_segment(
                            [row.left_bottom(), row.right_bottom()],
                            Stroke::new(1.0_f32, LINE.gamma_multiply(0.6)),
                        );
                    }
                    if let Some(track) = add_track {
                        let target = Rect::from_min_size(
                            Pos2::new(rect.left(), rect.top() + occupied_count as f32 * row_height),
                            Vec2::new(width, row_height),
                        )
                        .intersect(canvas_clip);
                        if target.height() >= 22.0 {
                            painter.text(
                                target.center(),
                                Align2::CENTER_CENTER,
                                format!("Drop a sound to add track {}", track + 1),
                                FontId::proportional(13.0),
                                MUTED,
                            );
                        }
                    }
                    let visible = ui.clip_rect().intersect(rect);
                    let first_bar = ((visible.left() - rect.left()) / bar_width)
                        .floor()
                        .max(0.0) as u32;
                    let last_bar = ((visible.right() - rect.left()) / bar_width)
                        .ceil()
                        .min(bars as f32) as u32;
                    for bar in first_bar..=last_bar {
                        let x = rect.left() + bar as f32 * bar_width;
                        painter.line_segment(
                            [Pos2::new(x, rect.top()), Pos2::new(x, rect.bottom())],
                            Stroke::new(
                                1.0_f32,
                                if bar % 4 == 0 {
                                    LINE
                                } else {
                                    Color32::from_rgb(35, 40, 46)
                                },
                            ),
                        );
                    }
                    for clip in &self.session.project().clips {
                        let clip_rect = self.clip_rect(clip, rect);
                        if visible.intersects(clip_rect) {
                            let sample = self.session.library().sample_by_id(clip.sample_id);
                            draw_clip(
                                &painter,
                                clip_rect,
                                sample,
                                self.selected_clips.contains(&clip.id),
                                clip.gain,
                            );
                        }
                    }
                    let play_x = rect.left()
                        + self.session.cursor_tick() as f32 / TICKS_PER_BAR as f32 * bar_width;
                    painter.line_segment(
                        [
                            Pos2::new(play_x, rect.top()),
                            Pos2::new(play_x, rect.bottom()),
                        ],
                        Stroke::new(1.7_f32, ACCENT),
                    );
                    if (response.clicked() || response.double_clicked())
                        && let Some(pos) = response.interact_pointer_pos()
                    {
                        let hit = self
                            .session
                            .project()
                            .clips
                            .iter()
                            .rev()
                            .find(|c| self.clip_rect(c, rect).contains(pos))
                            .cloned();
                        if let Some(clip) = hit {
                            if self.erase {
                                let result = self.session.delete(clip.id);
                                self.apply(result);
                            } else {
                                self.select_clip(clip.id, ui.input(|i| i.modifiers.shift));
                                if response.double_clicked() {
                                    self.preview(clip.sample_id);
                                }
                            }
                        } else if let Some(sample) = self
                            .selected_sample
                            .filter(|_| !ui.input(|i| i.modifiers.shift))
                        {
                            let Some(track) = self.timeline_track_at(pos) else {
                                return;
                            };
                            let tick =
                                self.position_tick(pos.x, rect, ui.input(|i| i.modifiers.ctrl));
                            self.place(sample, track, tick);
                        } else {
                            self.selected_clip = None;
                            if !ui.input(|i| i.modifiers.shift) {
                                self.selected_clips.clear();
                            }
                        }
                    }
                    if response.drag_started()
                        && let Some(pos) = ui.input(|i| i.pointer.press_origin())
                    {
                        if let Some(clip) = self
                            .session
                            .project()
                            .clips
                            .iter()
                            .rev()
                            .find(|c| self.clip_rect(c, rect).contains(pos))
                            .cloned()
                        {
                            let offset = self
                                .position_tick(pos.x, rect, true)
                                .saturating_sub(clip.start_tick);
                            if !self.selected_clips.contains(&clip.id) {
                                self.select_clip(clip.id, false);
                            }
                            self.drag = Some(DragItem::Clip {
                                id: clip.id,
                                offset,
                                copy: ui.input(|i| i.modifiers.alt || i.pointer.secondary_down()),
                            });
                        } else {
                            self.selection_origin = Some(pos);
                            self.selection_base = if ui.input(|i| i.modifiers.shift) {
                                self.selected_clips.clone()
                            } else {
                                BTreeSet::new()
                            };
                        }
                    }
                    if let Some(origin) = self.selection_origin {
                        if let Some(pos) = ui.input(|i| i.pointer.latest_pos()) {
                            let selection = Rect::from_two_pos(origin, pos).intersect(rect);
                            painter.rect_filled(
                                selection,
                                0.0,
                                Color32::from_rgba_unmultiplied(123, 170, 246, 35),
                            );
                            outline(&painter, selection, TEXT, 1.0);
                            self.selected_clips = self.selection_base.clone();
                            let hits: Vec<_> = self
                                .session
                                .project()
                                .clips
                                .iter()
                                .filter(|c| self.clip_rect(c, rect).intersects(selection))
                                .map(|c| c.id)
                                .collect();
                            self.selected_clips.extend(hits);
                            self.selected_clip = self.selected_clips.iter().next().copied();
                        }
                        if response.drag_stopped() || ui.input(|i| i.pointer.any_released()) {
                            self.selection_origin = None;
                        }
                    }
                },
            );
            let labels = Rect::from_min_size(
                Pos2::new(body_left, whole.top()),
                Vec2::new(label_width, track_height),
            );
            ui.scope_builder(
                egui::UiBuilder::new()
                    .max_rect(labels)
                    .id_salt("track_labels"),
                |ui| {
                    ui.set_clip_rect(labels.intersect(ui.clip_rect()));
                    ui.spacing_mut().item_spacing.y = 0.0;
                    for &index in &visible_tracks {
                        let (rect, _) =
                            ui.allocate_exact_size(Vec2::new(74.0, row_height), Sense::hover());
                        if add_track == Some(index) {
                            ui.painter().text(
                                rect.center(),
                                Align2::CENTER_CENTER,
                                format!("Add {}", index + 1),
                                FontId::proportional(13.0),
                                TEXT,
                            );
                            let response = ui.interact(
                                rect,
                                ui.id().with(("add_track", index)),
                                Sense::click(),
                            );
                            if response.clicked() {
                                self.armed_track = index;
                                self.message(format!("Recording will create track {}.", index + 1));
                            }
                            response.on_hover_text(
                                "Drop a sound on this row or select it for recording.",
                            );
                            continue;
                        }
                        ui.painter().rect_filled(rect.shrink(1.0), 4.0, PANEL);
                        ui.painter().rect_filled(
                            Rect::from_center_size(
                                rect.center(),
                                Vec2::new(25.0, row_height - 3.0),
                            ),
                            0.0,
                            PANEL,
                        );
                        ui.painter().text(
                            rect.center() + Vec2::new(0.0, -1.0),
                            Align2::CENTER_CENTER,
                            format!("{}", index + 1),
                            FontId::proportional(14.0),
                            Color32::WHITE,
                        );
                        let arm = ui.interact(
                            Rect::from_center_size(rect.center(), Vec2::new(28.0, row_height)),
                            ui.id().with(("arm", index)),
                            Sense::click(),
                        );
                        if self.armed_track == index {
                            ui.painter().circle_filled(
                                rect.center() + Vec2::new(0.0, 9.0),
                                2.0,
                                CORAL,
                            );
                        }
                        if arm.clicked() {
                            self.armed_track = index;
                            self.message(format!("Track {} is ready for recording.", index + 1));
                        }
                        arm.on_hover_text("Select this track for recording");
                        let track = &self.session.project().tracks[index];
                        for (left, enabled) in
                            [(true, track.left_enabled), (false, track.right_enabled)]
                        {
                            let x = if left {
                                rect.left() + 11.0
                            } else {
                                rect.right() - 11.0
                            };
                            let button = Rect::from_center_size(
                                Pos2::new(x, rect.center().y),
                                Vec2::new(19.0, row_height - 3.0),
                            );
                            let response = ui.interact(
                                button,
                                ui.id().with(("route", index, left)),
                                Sense::click(),
                            );
                            speaker(ui.painter(), button, enabled, left);
                            if response.clicked() {
                                route_change = Some((index, left, !enabled));
                            }
                            response.on_hover_text(if left {
                                "Left output"
                            } else {
                                "Right output"
                            });
                        }
                    }
                },
            );
        });
        let ruler_clip = Rect::from_min_max(
            ruler_strip.min + Vec2::new(lane_inset, 0.0),
            Pos2::new(scroll.inner_rect.right(), ruler_strip.bottom()),
        );
        self.timeline_ruler_rect = Some(ruler_clip);
        let ruler = Rect::from_min_size(
            ruler_strip.min + Vec2::new(lane_inset - painted_offset_x, 0.0),
            Vec2::new(width, 26.0),
        );
        let ruler_response = ui.interact(
            ruler_clip,
            ui.id().with("bar_ruler"),
            Sense::click_and_drag(),
        );
        let ruler_painter = ui.painter().with_clip_rect(ruler_clip);
        ruler_painter.rect_filled(ruler, 0.0, PANEL);
        if let Some([start, end]) = self.session.project().loop_region {
            let rect = Rect::from_min_max(
                Pos2::new(
                    ruler.left() + start as f32 / TICKS_PER_BAR as f32 * bar_width,
                    ruler.top(),
                ),
                Pos2::new(
                    ruler.left() + end as f32 / TICKS_PER_BAR as f32 * bar_width,
                    ruler.bottom(),
                ),
            );
            ruler_painter.rect_filled(
                rect,
                0.0,
                Color32::from_rgba_unmultiplied(184, 227, 126, 35),
            );
        }
        let first_bar = ((ruler_clip.left() - ruler.left()) / bar_width)
            .floor()
            .max(0.0) as u32;
        let last_bar = ((ruler_clip.right() - ruler.left()) / bar_width)
            .ceil()
            .min(bars as f32) as u32;
        for bar in first_bar..last_bar {
            let x = ruler.left() + bar as f32 * bar_width;
            ruler_painter.text(
                Pos2::new(x + bar_width * 0.5, ruler.center().y),
                Align2::CENTER_CENTER,
                format!("{}", bar + 1),
                FontId::proportional(13.0),
                if bar == self.session.cursor_tick() / TICKS_PER_BAR {
                    CORAL
                } else {
                    TEXT
                },
            );
        }
        if ruler_response.clicked()
            && let Some(pos) = ruler_response.interact_pointer_pos()
        {
            self.seek(self.position_tick(pos.x, ruler, false));
        }
        if ruler_response.drag_started() {
            self.loop_start = ui
                .input(|i| i.pointer.press_origin())
                .map(|p| self.position_tick(p.x, ruler, false));
        }
        if ruler_response.drag_stopped()
            && let (Some(start), Some(pos)) = (
                self.loop_start.take(),
                ruler_response.interact_pointer_pos(),
            )
        {
            let end = self.position_tick(pos.x, ruler, false);
            let region = [start.min(end), start.max(end).saturating_add(TICKS_PER_BAR)];
            let result = self.session.edit(EditCommand::SetLoop(Some(region)));
            self.apply(result);
        }
        if let Some((index, left, enabled)) = route_change {
            let result = self.session.edit(EditCommand::SetRouting {
                track: index,
                left,
                enabled,
            });
            self.apply(result);
        }
        ui.add_space(3.0);
        self.selected_inspector(ui);
    }

    pub(super) fn draw(&mut self, ctx: &egui::Context) {
        self.poll_application(ctx);
        if ctx.input(|i| i.viewport().close_requested())
            && !self.allow_close
            && (self.session.unsaved()
                || self.session.has_recorded_take()
                || self.session.is_recording())
        {
            ctx.send_viewport_cmd(egui::ViewportCommand::CancelClose);
            self.request_action(PendingAction::Close, ctx);
        }
        if self.pending_action.is_none() {
            self.keyboard(ctx);
        }
        self.header(ctx);
        self.side_rail(ctx);
        self.bottom(ctx);
        egui::CentralPanel::default()
            .frame(
                egui::Frame::new()
                    .fill(CANVAS)
                    .inner_margin(egui::Margin::symmetric(18, 14)),
            )
            .show(ctx, |ui| self.timeline(ui));
        self.finish_drag(ctx);
        self.dialogs(ctx);
        self.tool_windows(ctx);
        self.capture(ctx);
    }
}

/// Keep the viewport still while the playhead is comfortably visible.
fn follow_offset(
    playhead: f32,
    offset: f32,
    visible_width: f32,
    content_width: f32,
) -> Option<f32> {
    let margin = (visible_width * 0.1).min(48.0);
    if playhead >= offset + margin && playhead <= offset + visible_width - margin {
        return None;
    }
    let next =
        (playhead - visible_width * 0.25).clamp(0.0, (content_width - visible_width).max(0.0));
    ((next - offset).abs() > 0.5).then_some(next)
}

fn transport_position(ui: &mut egui::Ui, tick: u32) -> Response {
    let bars = tick / TICKS_PER_BAR + 1;
    let beat = (tick % TICKS_PER_BAR) / (TICKS_PER_BAR / 4) + 1;
    let text = format!("{bars:03} : {beat}");
    let (rect, response) = ui.allocate_exact_size(Vec2::new(112.0, 42.0), Sense::hover());
    ui.painter().rect_filled(rect, 6.0, CANVAS);
    ui.painter()
        .with_clip_rect(rect.intersect(ui.clip_rect()))
        .text(
            rect.center(),
            Align2::CENTER_CENTER,
            &text,
            FontId::new(23.0, egui::FontFamily::Name("Space Grotesk Bold".into())),
            TEXT,
        );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
    response.on_hover_text(format!("Bar {bars}, beat {beat}"))
}

pub(super) fn effects_controls(ui: &mut egui::Ui, state: &mut EffectsState) {
    ui.horizontal(|ui| {
        let mut enabled = state.filter.is_some();
        if ui.checkbox(&mut enabled, "Filter").changed() {
            state.filter = enabled.then(Default::default);
        }
        if let Some(filter) = &mut state.filter {
            ui.add(
                egui::Slider::new(&mut filter.cutoff_hz, 20.0..=20000.0)
                    .logarithmic(true)
                    .text("Cutoff"),
            );
        }
    });
    if let Some(filter) = &mut state.filter {
        ui.add(egui::Slider::new(&mut filter.resonance, 0.0..=1.0).text("Resonance"));
        ui.add(egui::Slider::new(&mut filter.drive, 0.0..=1.0).text("Overdrive"));
        ui.add(egui::Slider::new(&mut filter.wet, 0.0..=1.0).text("Filter volume"));
    }
    let mut enabled = state.echo.is_some();
    if ui.checkbox(&mut enabled, "Echo").changed() {
        state.echo = enabled.then(Default::default);
    }
    if let Some(echo) = &mut state.echo {
        ui.add(egui::Slider::new(&mut echo.delay_beats, 0.001..=2.0).text("Delay (beats)"));
        ui.add(egui::Slider::new(&mut echo.feedback, 0.0..=0.95).text("Feedback"));
        ui.add(egui::Slider::new(&mut echo.wet, 0.0..=1.0).text("Echo volume"));
    }
    let mut enabled = state.robotize.is_some();
    if ui.checkbox(&mut enabled, "Robotize").changed() {
        state.robotize = enabled.then(Default::default);
    }
    if let Some(robot) = &mut state.robotize {
        ui.add(
            egui::Slider::new(&mut robot.frequency_hz, 1.0..=2000.0)
                .logarithmic(true)
                .text("Color"),
        );
        ui.add(egui::Slider::new(&mut robot.wet, 0.0..=1.0).text("Robotize volume"));
    }
    ui.add(egui::Slider::new(&mut state.dirty, 0.0..=1.0).text("Dirty distortion"));
    ui.add(egui::Slider::new(&mut state.noise, 0.0..=1.0).text("Noise distortion"));
    ui.add(egui::Slider::new(&mut state.volume, 0.0..=2.0).text("Output volume"));
}

pub(super) fn pan_control(ui: &mut egui::Ui, index: usize, pan: &mut f32) -> Response {
    ui.add(egui::Slider::new(pan, -1.0..=1.0).show_value(false))
        .on_hover_text(format!("Track {} pan: {:.0}%", index + 1, *pan * 100.0))
}

pub(super) fn bpm_picker(ui: &mut egui::Ui, id: &str, bpm: &mut f64) -> Response {
    let before = *bpm;
    let mut response = egui::ComboBox::from_id_salt(id)
        .selected_text(format!("{bpm:.0}"))
        .width(64.0)
        .show_ui(ui, |ui| {
            for value in SUPPORTED_BPMS {
                ui.selectable_value(bpm, value, format!("{value:.0}"));
            }
        })
        .response;
    if *bpm != before {
        response.mark_changed();
    }
    response
}

pub(super) fn recording_destination(
    ui: &mut egui::Ui,
    selected: &mut usize,
    occupied: &[usize],
) -> Response {
    egui::ComboBox::from_id_salt("record_destination")
        .selected_text(format!("{}", *selected + 1))
        .width(48.0)
        .show_ui(ui, |ui| {
            for index in 0..TRACK_COUNT {
                let label = if occupied.contains(&index) {
                    format!("Track {}", index + 1)
                } else {
                    format!("New track {}", index + 1)
                };
                ui.selectable_value(selected, index, label);
            }
        })
        .response
        .on_hover_text(
            "Choose a track for recording. Unused tracks appear only when you add a sound.",
        )
}

pub(super) fn configure_style(ctx: &egui::Context) {
    configure_fonts(ctx);
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    style.visuals.override_text_color = Some(TEXT);
    style.visuals.panel_fill = CANVAS;
    style.visuals.window_fill = PANEL;
    style.visuals.window_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.window_corner_radius = egui::CornerRadius::same(10);
    style.visuals.faint_bg_color = RAISED;
    style.visuals.extreme_bg_color = CANVAS;
    style.visuals.selection.bg_fill = Color32::from_rgb(65, 85, 50);
    style.visuals.selection.stroke = Stroke::new(1.0_f32, ACCENT);
    style.visuals.hyperlink_color = ACCENT;
    style.visuals.warn_fg_color = Color32::from_rgb(232, 197, 133);
    style.visuals.error_fg_color = CORAL;
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(5);
        widget.fg_stroke = Stroke::new(1.0_f32, TEXT);
        widget.bg_stroke = Stroke::new(1.0_f32, LINE);
        widget.bg_fill = RAISED;
        widget.weak_bg_fill = RAISED;
    }
    style.visuals.widgets.hovered.bg_fill = HOVER;
    style.visuals.widgets.hovered.weak_bg_fill = HOVER;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, MUTED);
    style.visuals.widgets.active.bg_fill = HOVER;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, ACCENT);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, LINE);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, MUTED);
    style.spacing.item_spacing = Vec2::new(8.0, 6.0);
    style.spacing.button_padding = Vec2::new(10.0, 6.0);
    style.spacing.interact_size = Vec2::new(32.0, 28.0);
    style.spacing.slider_width = 84.0;
    style
        .text_styles
        .insert(egui::TextStyle::Body, FontId::proportional(13.0));
    style.text_styles.insert(
        egui::TextStyle::Button,
        FontId::new(13.0, egui::FontFamily::Name("Space Grotesk Medium".into())),
    );
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(20.0, egui::FontFamily::Name("Space Grotesk Bold".into())),
    );
    ctx.set_style(style);
}

pub(super) fn outline(painter: &egui::Painter, rect: Rect, color: Color32, width: f32) {
    let stroke = Stroke::new(width, color);
    painter.line_segment([rect.left_top(), rect.right_top()], stroke);
    painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.right_bottom(), rect.left_bottom()], stroke);
    painter.line_segment([rect.left_bottom(), rect.left_top()], stroke);
}

pub(super) fn action_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(if active { CANVAS } else { TEXT }))
            .fill(if active { ACCENT } else { RAISED })
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(48.0, 30.0)),
    )
}

pub(super) fn rail_button(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    active: bool,
) -> Response {
    let width = 48.0_f32.min(ui.available_width().max(1.0));
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 30.0), Sense::click());
    let fill = if active {
        HOVER
    } else if response.hovered() {
        RAISED
    } else {
        CANVAS
    };
    ui.painter().rect_filled(rect, 5.0, fill);
    if active || response.has_focus() {
        ui.painter().rect_stroke(
            rect.shrink(0.5),
            5.0,
            Stroke::new(1.0_f32, ACCENT),
            egui::StrokeKind::Inside,
        );
    }
    let color = if !ui.is_enabled() {
        MUTED.gamma_multiply(0.5)
    } else if active {
        ACCENT
    } else {
        TEXT
    };
    ui.painter().image(
        icons.texture_id(icon),
        Rect::from_center_size(rect.center(), Vec2::splat(20.0)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        color,
    );
    response.widget_info(|| {
        if icon.is_toggle() {
            egui::WidgetInfo::selected(
                egui::WidgetType::SelectableLabel,
                ui.is_enabled(),
                active,
                icon.label(),
            )
        } else {
            egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), icon.label())
        }
    });
    response.on_hover_text(icon.label())
}

pub(super) fn rail_heading(ui: &mut egui::Ui, icons: &RailIcons, icon: RailIcon) {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(48.0, 20.0), Sense::hover());
    ui.painter().image(
        icons.texture_id(icon),
        Rect::from_center_size(rect.center(), Vec2::splat(18.0)),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        MUTED,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), icon.label())
    });
    response.on_hover_text(icon.label());
}

pub(super) fn category_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    ui.add(
        egui::Button::new(egui::RichText::new(label).color(if active { TEXT } else { MUTED }))
            .fill(if active { HOVER } else { PANEL })
            .stroke(Stroke::new(1.0_f32, if active { MUTED } else { PANEL }))
            .min_size(Vec2::new(40.0, 26.0)),
    )
    .on_hover_text(format!("Show {label} sounds"))
}

pub(super) fn group_color(group: &str) -> Color32 {
    match group.to_ascii_lowercase().as_str() {
        "loop" => Color32::from_rgb(212, 193, 130),
        "drum" => Color32::from_rgb(217, 157, 120),
        "bass" => Color32::from_rgb(162, 196, 133),
        "sequence" => Color32::from_rgb(137, 176, 210),
        "sphere" | "wave" => Color32::from_rgb(127, 190, 185),
        "effect" => Color32::from_rgb(217, 145, 146),
        "voice" => Color32::from_rgb(185, 158, 203),
        _ => Color32::from_rgb(187, 179, 159),
    }
}

pub(super) fn sample_button(ui: &mut egui::Ui, sample: &Sample, selected: bool) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(148.0, 46.0), Sense::click_and_drag());
    let color = group_color(&sample.group_name);
    let painter = ui.painter_at(rect);
    painter.rect_filled(
        rect,
        5.0,
        if response.hovered() || selected {
            HOVER
        } else {
            RAISED
        },
    );
    if selected || response.has_focus() {
        painter.rect_stroke(
            rect.shrink(0.5),
            5.0,
            Stroke::new(1.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
    let text = painter.layout_no_wrap(sample.name.clone(), FontId::proportional(12.0), TEXT);
    let clip = painter.with_clip_rect(rect.shrink2(Vec2::new(10.0, 3.0)));
    clip.galley(rect.min + Vec2::new(10.0, 5.0), text, TEXT);
    clip.circle_filled(rect.min + Vec2::new(12.5, 32.0), 2.5, color);
    clip.text(
        rect.min + Vec2::new(21.0, 32.0),
        Align2::LEFT_CENTER,
        format!(
            "{}  /  {} bar{}",
            sample.group_name,
            sample.bars,
            if sample.bars == 1 { "" } else { "s" }
        ),
        FontId::proportional(10.0),
        MUTED,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &sample.name)
    });
    response.on_hover_text(format!(
        "{}\n{} bars | {} Hz\nDouble-click to listen. Drag to a track.",
        sample.name, sample.bars, sample.sample_rate
    ))
}

pub(super) fn draw_clip(
    painter: &egui::Painter,
    rect: Rect,
    sample: Option<&Sample>,
    selected: bool,
    gain: f32,
) {
    let color = sample.map(|s| group_color(&s.group_name)).unwrap_or(MUTED);
    painter.rect_filled(rect, 3.0, color);
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)));
    if selected {
        painter.rect_stroke(
            rect.shrink(0.5),
            3.0,
            Stroke::new(2.0_f32, TEXT),
            egui::StrokeKind::Inside,
        );
    }
    if let Some(sample) = sample {
        let size = if rect.height() < 28.0 { 10.0 } else { 12.0 };
        let text = clipped.layout(
            visible_clip_label(
                &sample.name,
                rect.width() - 12.0,
                size,
                if rect.height() < 38.0 {
                    20.0
                } else {
                    rect.height()
                },
            ),
            FontId::proportional(size),
            CANVAS,
            (rect.width() - 12.0).max(1.0),
        );
        clipped.galley(rect.left_top() + Vec2::new(6.0, 4.0), text, CANVAS);
    }
    if selected {
        let y = rect.bottom() - (gain.min(2.0) / 2.0 * rect.height()).max(2.0);
        clipped.line_segment(
            [
                Pos2::new(rect.left() + 4.0, y),
                Pos2::new(rect.right() - 4.0, y),
            ],
            Stroke::new(1.0_f32, Color32::from_black_alpha(100)),
        );
    }
}

pub(super) fn clip_label(name: &str, width: f32, font_size: f32) -> String {
    let capacity = (width / (font_size * 0.51)).floor().max(1.0) as usize;
    if name.chars().count() <= capacity {
        return name.into();
    }
    let split = name
        .char_indices()
        .enumerate()
        .take(capacity + 1)
        .filter(|(_, (_, c))| c.is_whitespace())
        .last()
        .map(|(_, (byte, _))| byte)
        .unwrap_or_else(|| {
            name.char_indices()
                .nth(capacity)
                .map_or(name.len(), |(byte, _)| byte)
        });
    let (first, tail) = name.split_at(split);
    let tail = tail.trim_start();
    let second = if tail.chars().count() > capacity {
        format!(
            "{}…",
            tail.chars()
                .take(capacity.saturating_sub(1))
                .collect::<String>()
        )
    } else {
        tail.into()
    };
    format!("{first}\n{second}")
}

pub(super) fn visible_clip_label(name: &str, width: f32, font_size: f32, height: f32) -> String {
    if height >= 24.0 {
        return clip_label(name, width, font_size);
    }
    let capacity = (width / (font_size * 0.51)).floor().max(1.0) as usize;
    if name.chars().count() <= capacity {
        name.into()
    } else {
        format!(
            "{}…",
            name.chars()
                .take(capacity.saturating_sub(1))
                .collect::<String>()
        )
    }
}

pub(super) fn route_button(ui: &mut egui::Ui, text: &str, active: &mut bool) -> Response {
    let mut response = ui.add(
        egui::Button::new(egui::RichText::new(text).color(if *active { CANVAS } else { TEXT }))
            .fill(if *active { ACCENT } else { RAISED })
            .min_size(Vec2::new(17.0, 20.0)),
    );
    if response.clicked() {
        *active = !*active;
        response.mark_changed();
    }
    response
}

pub(super) fn speaker(painter: &egui::Painter, rect: Rect, enabled: bool, left: bool) {
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        if left { "L" } else { "R" },
        FontId::proportional(10.0),
        if enabled { MUTED } else { LINE },
    );
}

#[derive(Clone, Copy)]
pub(super) enum TransportIcon {
    Loop,
    Start,
    Back,
    Stop,
    Pause,
    Play,
    Forward,
    End,
    Record,
}

pub(super) fn transport_button(
    ui: &mut egui::Ui,
    icon: TransportIcon,
    active: bool,
    tooltip: &str,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(42.0, 42.0), Sense::click());
    let primary = matches!(icon, TransportIcon::Play | TransportIcon::Pause);
    let fill = if primary {
        ACCENT
    } else if response.hovered() {
        HOVER
    } else {
        RAISED
    };
    ui.painter().rect_filled(rect, 6.0, fill);
    let center = rect.center();
    let painter = ui.painter();
    let color = if primary {
        CANVAS
    } else if active {
        ACCENT
    } else {
        TEXT
    };
    let triangle = |x: f32, direction: f32| {
        painter.add(egui::Shape::convex_polygon(
            vec![
                center + Vec2::new(x - direction * 5.0, -9.0),
                center + Vec2::new(x - direction * 5.0, 9.0),
                center + Vec2::new(x + direction * 7.0, 0.0),
            ],
            color,
            Stroke::NONE,
        ));
    };
    match icon {
        TransportIcon::Play => triangle(0.0, 1.0),
        TransportIcon::Pause => {
            for x in [-5.0, 5.0] {
                painter.rect_filled(
                    Rect::from_center_size(center + Vec2::new(x, 0.0), Vec2::new(5.0, 18.0)),
                    0.0,
                    color,
                );
            }
        }
        TransportIcon::Stop => {
            painter.rect_filled(
                Rect::from_center_size(center, Vec2::splat(17.0)),
                0.0,
                color,
            );
        }
        TransportIcon::Record => {
            painter.circle_filled(center, 10.0, CORAL);
            if active {
                painter.circle_stroke(center, 13.0, Stroke::new(1.0_f32, Color32::WHITE));
            }
        }
        TransportIcon::Back => {
            triangle(-5.0, -1.0);
            triangle(7.0, -1.0);
        }
        TransportIcon::Forward => {
            triangle(-7.0, 1.0);
            triangle(5.0, 1.0);
        }
        TransportIcon::Start => {
            triangle(3.0, -1.0);
            painter.line_segment(
                [
                    center + Vec2::new(-10.0, -10.0),
                    center + Vec2::new(-10.0, 10.0),
                ],
                Stroke::new(3.0_f32, color),
            );
        }
        TransportIcon::End => {
            triangle(-3.0, 1.0);
            painter.line_segment(
                [
                    center + Vec2::new(10.0, -10.0),
                    center + Vec2::new(10.0, 10.0),
                ],
                Stroke::new(3.0_f32, color),
            );
        }
        TransportIcon::Loop => {
            let points: Vec<Pos2> = (0..=20)
                .map(|i| {
                    let a = 0.4 + i as f32 / 20.0 * 5.3;
                    center + Vec2::new(a.cos() * 12.0, a.sin() * 8.0)
                })
                .collect();
            painter.add(egui::Shape::line(points, Stroke::new(2.2_f32, color)));
            painter.add(egui::Shape::convex_polygon(
                vec![
                    center + Vec2::new(11.0, 3.0),
                    center + Vec2::new(4.0, 2.0),
                    center + Vec2::new(10.0, -4.0),
                ],
                color,
                Stroke::NONE,
            ));
        }
    }
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(2.0),
            6.0,
            Stroke::new(2.0_f32, if primary { CANVAS } else { ACCENT }),
            egui::StrokeKind::Inside,
        );
    }
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), tooltip)
    });
    response.on_hover_text(tooltip)
}

fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let fallback = fonts.families[&egui::FontFamily::Proportional].clone();
    let families: [(&str, [&[u8]; 3]); 3] = [
        (
            "Space Grotesk",
            [
                include_bytes!("../../assets/fonts/SpaceGrotesk-Regular-latin.ttf"),
                include_bytes!("../../assets/fonts/SpaceGrotesk-Regular-latin-ext.ttf"),
                include_bytes!("../../assets/fonts/SpaceGrotesk-Regular-vietnamese.ttf"),
            ],
        ),
        (
            "Space Grotesk Medium",
            [
                include_bytes!("../../assets/fonts/SpaceGrotesk-Medium-latin.ttf"),
                include_bytes!("../../assets/fonts/SpaceGrotesk-Medium-latin-ext.ttf"),
                include_bytes!("../../assets/fonts/SpaceGrotesk-Medium-vietnamese.ttf"),
            ],
        ),
        (
            "Space Grotesk Bold",
            [
                include_bytes!("../../assets/fonts/SpaceGrotesk-Bold-latin.ttf"),
                include_bytes!("../../assets/fonts/SpaceGrotesk-Bold-latin-ext.ttf"),
                include_bytes!("../../assets/fonts/SpaceGrotesk-Bold-vietnamese.ttf"),
            ],
        ),
    ];
    for (family, subsets) in families {
        let mut names = Vec::new();
        for (index, bytes) in subsets.into_iter().enumerate() {
            let name = format!("{family}-{index}");
            fonts
                .font_data
                .insert(name.clone(), egui::FontData::from_static(bytes).into());
            names.push(name);
        }
        names.extend(fallback.clone());
        fonts
            .families
            .insert(egui::FontFamily::Name(family.into()), names.clone());
        if family == "Space Grotesk" {
            fonts.families.insert(egui::FontFamily::Proportional, names);
        }
    }
    ctx.set_fonts(fonts);
}
