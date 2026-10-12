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
pub(super) const MUTED: Color32 = Color32::from_rgb(164, 174, 182);
const LINE: Color32 = Color32::from_rgb(58, 65, 72);
pub(super) const RAISED: Color32 = Color32::from_rgb(39, 45, 51);
const HOVER: Color32 = Color32::from_rgb(51, 59, 66);
pub(super) const DEFAULT_STATUS: &str = "Ready";
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

pub(super) fn title_text(text: &str) -> String {
    deunicode::deunicode(text).to_ascii_uppercase()
}

pub(super) fn heading(text: impl Into<String>, size: f32) -> egui::RichText {
    egui::RichText::new(title_text(&text.into()))
        .font(FontId::new(
            size,
            egui::FontFamily::Name("Syncopate Bold".into()),
        ))
        .color(TEXT)
}

/// Name a control whose visible label is separate text, for assistive
/// technology and agents.
pub(super) fn name_control(response: Response, name: &str) -> Response {
    response
        .ctx
        .accesskit_node_builder(response.id, |node| node.set_label(name));
    response
}

/// Group the controls of a modal under its title, for assistive technology
/// and agents.
pub(super) fn name_dialog(ui: &egui::Ui, title: &str) {
    ui.ctx().accesskit_node_builder(ui.unique_id(), |node| {
        node.set_role(egui::accesskit::Role::Dialog);
        node.set_label(title);
    });
}

impl MixJamApp {
    /// Load the bundled artwork once. A failure is reported once; the header
    /// then falls back to text actions and the tool rail stays hidden.
    fn icons(&mut self, ctx: &egui::Context) -> Option<Arc<RailIcons>> {
        if self.rail_icons.as_ref().is_none_or(|loaded| {
            loaded
                .as_ref()
                .is_ok_and(|icons| !icons.matches_density(ctx))
        }) {
            let loaded = RailIcons::load(ctx).map(Arc::new);
            if let Err(error) = &loaded {
                self.error(error.clone());
            }
            self.rail_icons = Some(loaded);
        }
        self.rail_icons.as_ref()?.as_ref().ok().cloned()
    }

    pub(super) fn header(&mut self, ctx: &egui::Context) {
        let icons = self.icons(ctx);
        let action = |ui: &mut egui::Ui, icon: RailIcon, label: &str| match &icons {
            Some(icons) => action_icon_button(ui, icons, icon, label),
            None => action_button(ui, label, false).on_hover_text(label),
        };
        egui::TopBottomPanel::top("header")
            .exact_height(66.0)
            .frame(
                egui::Frame::new()
                    .fill(CANVAS)
                    .inner_margin(egui::Margin::symmetric(22, 10)),
            )
            .show(ctx, |ui| {
                ui.horizontal_centered(|ui| {
                    if let Some(icons) = &icons {
                        icons.brand_image(ui);
                    }
                    ui.label(heading("MixJam", 27.0));
                    ui.label(heading("Techno", 14.0).color(MUTED));
                    ui.add_space(24.0);
                    ui.separator();
                    ui.add_space(18.0);
                    let title = if self.session.unsaved() {
                        format!("{} *", self.session.project().title)
                    } else {
                        self.session.project().title.clone()
                    };
                    let title_width = (ui.available_width() - 190.0).max(40.0);
                    ui.add_sized(
                        [title_width, 30.0],
                        egui::Label::new(heading(title, 15.0)).truncate(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if action(ui, RailIcon::Libraries, "Help").clicked() {
                            self.show_help = !self.show_help;
                        }
                        ui.add_space(8.0);
                        if action(ui, RailIcon::Redo, "Redo").clicked() {
                            self.history_step(true);
                        }
                        if action(ui, RailIcon::Undo, "Undo").clicked() {
                            self.history_step(false);
                        }
                    });
                });
            });
    }

    pub(super) fn side_rail(&mut self, ctx: &egui::Context) {
        let Some(icons) = self.icons(ctx) else {
            return;
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
                    if rail_button(
                        ui,
                        &icons,
                        RailIcon::Mixer,
                        self.bottom_module == BottomModule::Mixer,
                    )
                    .clicked()
                    {
                        self.bottom_module = BottomModule::Mixer;
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
                        self.stretch_source_bpm = self.source_bpm().clamp(30.0, 300.0);
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
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.add(
                        egui::Hyperlink::from_label_and_url(
                            egui::RichText::new(format!("v{}", mixjam_techno::application::APP_VERSION))
                                .size(12.0)
                                .color(ACCENT),
                            "https://github.com/satyrlord/mixjam-techno",
                        )
                        .open_in_new_tab(true),
                    )
                    .on_hover_text("Open MixJam Techno on GitHub");
                    ui.with_layout(egui::Layout::left_to_right(egui::Align::Center), |ui| {
                        if !self.session.sample_conflicts().is_empty()
                            && ui.small_button("Sample conflicts").clicked()
                        {
                            self.show_sample_conflicts = true;
                            self.sample_conflict_error = None;
                        }
                        if (!self.session.project().import_notices.is_empty()
                            || !self.session.project().source_notices.is_empty()
                            || !self.session.missing_samples().is_empty())
                            && ui.small_button("Import details").clicked()
                        {
                            self.show_import_details = true;
                        }
                        if !self.session.missing_samples().is_empty() {
                            ui.label(egui::RichText::new(format!("{} missing · silent", self.session.missing_samples().len())).size(12.0).color(CORAL))
                                .on_hover_text("Missing samples play silence. Open Import details to see their saved references.");
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
            });
        let height = (ctx.content_rect().height() * 0.28).clamp(220.0, 290.0);
        // Reserve the transport and a usable arrangement, including its inspector.
        let min_height = self.bottom_module.minimum_height();
        // Keep the resized section stable when switching to a shorter module.
        let shared_minimum = BottomModule::ALL
            .into_iter()
            .map(BottomModule::minimum_height)
            .fold(0.0_f32, f32::max);
        let max_height = (ctx.available_rect().height() - 68.0 - 220.0).max(shared_minimum);
        let sounds = egui::TopBottomPanel::bottom("sounds")
            .default_height(height)
            .height_range(min_height..=max_height)
            .resizable(true)
            .frame(
                egui::Frame::new()
                    .fill(PANEL)
                    .inner_margin(egui::Margin::symmetric(18, 12)),
            )
            .show(ctx, |ui| {
                ui.set_min_height(ui.available_height());
                let row_spacing = ui.spacing().item_spacing.y;
                ui.spacing_mut().item_spacing.y = 0.0;
                self.bottom_toolbar(ui);
                ui.spacing_mut().item_spacing.y = row_spacing;
                match self.bottom_module {
                    BottomModule::Mixer => self.mixer(ui),
                    BottomModule::Sounds => self.palette(ui),
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
            if name_control(bpm_picker(ui, "project_bpm", &mut bpm), "Project BPM").changed() {
                let result = self.session.edit(EditCommand::SetTempo(bpm));
                self.apply(result);
            }
            ui.add_space(10.0);
            ui.label("Zoom");
            name_control(
                ui.add(
                    egui::Slider::new(&mut self.zoom, 0.55..=2.0)
                        .show_value(false)
                        .clamping(egui::SliderClamping::Always),
                ),
                "Zoom",
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

    fn bottom_toolbar(&mut self, ui: &mut egui::Ui) {
        ui.push_id("bottom-module-toolbar", |ui| {
            ui.horizontal(|ui| {
                ui.spacing_mut().interact_size.y = 28.0;
                let button_padding = ui.spacing().button_padding;
                ui.spacing_mut().button_padding.y = 3.0;
                let response = egui::ComboBox::from_id_salt("bottom_module")
                    .selected_text(heading(self.bottom_module.label(), 16.0))
                    .width(136.0)
                    .show_ui(ui, |ui| {
                        for module in BottomModule::ALL {
                            if ui.selectable_value(&mut self.bottom_module, module, module.label()).clicked() {
                                ui.close();
                            }
                        }
                    }).response;
                name_control(response, "Bottom module")
                    .on_hover_text("Choose the module shown in this section. Sounds selects source audio; Mixer adjusts track levels, pan, polarity and outputs.");
                ui.spacing_mut().button_padding = button_padding;
                if self.bottom_module == BottomModule::Sounds { self.palette_filters(ui); }
            });
        });
    }

    fn palette_filters(&mut self, ui: &mut egui::Ui) {
        let (samples, groups) = self.palette_view();
        name_control(
            ui.add(
                egui::TextEdit::singleline(&mut self.search)
                    .hint_text("Find a sound")
                    .desired_width(170.0),
            ),
            "Find a sound",
        );
        let libraries = egui::ComboBox::from_id_salt("palette_product")
            .selected_text(self.palette_product.map_or("All libraries", Product::label))
            .width(145.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.palette_product, None, "All libraries");
                for product in Product::ALL {
                    ui.selectable_value(&mut self.palette_product, Some(product), product.label());
                }
            })
            .response;
        name_control(libraries, "Library filter");
        let group_label = self.group.as_deref().map_or("All sounds", |g| {
            if g.is_empty() { "Uncategorized" } else { g }
        });
        let categories = egui::ComboBox::from_id_salt("palette_category")
            .selected_text(group_label)
            .width(105.0)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.group, None, "All sounds");
                for name in groups.iter() {
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
            })
            .response;
        name_control(categories, "Category filter");
        let sorting = egui::ComboBox::from_id_salt("palette_sort")
            .selected_text(self.palette_sort.label())
            .width(105.0)
            .show_ui(ui, |ui| {
                for sort in PaletteSort::ALL {
                    ui.selectable_value(&mut self.palette_sort, sort, sort.label());
                }
            })
            .response;
        name_control(sorting, "Sort order");
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} sounds", samples.len()))
                    .size(12.0)
                    .color(MUTED),
            );
        });
    }

    pub(super) fn palette(&mut self, ui: &mut egui::Ui) {
        let (samples, _) = self.palette_view();
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
                    "No sounds"
                } else {
                    "No matching sounds"
                },
                17.0,
            ));
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
        // Tile width follows the bar count, so rows hold a varying number of tiles.
        let row_width = ui.available_width();
        let starts = {
            let library = self.session.library().samples;
            tile_rows(
                samples
                    .iter()
                    .map(|&index| tile_width(library[index].bars, row_width)),
                row_width,
            )
        };
        let spacing =
            std::mem::replace(&mut ui.spacing_mut().item_spacing, Vec2::new(TILE_GAP, 6.0));
        egui::ScrollArea::vertical()
            .id_salt("sample_palette")
            .max_height(ui.available_height())
            .show_rows(ui, 46.0, starts.len(), |ui, rows| {
                for row in rows {
                    let end = starts.get(row + 1).copied().unwrap_or(samples.len());
                    ui.horizontal(|ui| {
                        for &sample_index in &samples[starts[row]..end] {
                            let (id, response) = {
                                let sample = &self.session.library().samples[sample_index];
                                let product = self.sample_product(sample.id);
                                let response = ui
                                    .push_id(sample.id, |ui| {
                                        sample_button(
                                            ui,
                                            sample,
                                            product,
                                            self.selected_sample == Some(sample.id),
                                            tile_width(sample.bars, row_width),
                                        )
                                    })
                                    .inner;
                                if self.capture_view_is("sample-details")
                                    && self.selected_sample == Some(sample.id)
                                {
                                    response.show_tooltip_text(sample_details(sample, product));
                                }
                                (sample.id, response)
                            };
                            if response.clicked() {
                                self.selected_sample = Some(id);
                            }
                            if response.double_clicked() {
                                self.preview(id);
                            }
                            if response.drag_started() {
                                self.selected_sample = Some(id);
                                self.drag = Some(DragItem::Sample(id));
                            }
                        }
                    });
                }
            });
        ui.spacing_mut().item_spacing = spacing;
    }

    pub(super) fn mixer(&mut self, ui: &mut egui::Ui) {
        let mut tracks = self.session.project().tracks.clone();
        let mut master = self.session.project().master_gain;
        let changed = mixer::show(
            ui,
            self.session.project(),
            self.session.library(),
            &mut tracks,
            &mut master,
        );
        if changed {
            let result = self.session.edit(EditCommand::SetMixer {
                tracks,
                master_gain: master,
            });
            self.apply(result);
            ui.ctx().request_repaint();
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
            let mut change = name_control(
                ui.add(egui::DragValue::new(&mut gain).range(0.0..=2.0).speed(0.01)),
                "Clip gain",
            )
            .changed();
            ui.label("Trim start (frames)");
            change |= name_control(
                ui.add(
                    egui::DragValue::new(&mut offset)
                        .range(0..=max_frames.saturating_sub(1))
                        .speed(100),
                ),
                "Clip trim start (frames)",
            )
            .changed();
            ui.label("Length (bars)");
            let mut bars = length as f64 / TICKS_PER_BAR as f64;
            if name_control(
                ui.add(
                    egui::DragValue::new(&mut bars)
                        .range(1.0 / 16.0..=256.0)
                        .speed(1.0 / 16.0),
                ),
                "Clip length (bars)",
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
                    let response = name_control(
                        ui.interact(rect, ui.id().with("canvas"), Sense::click_and_drag()),
                        "Arrangement lanes",
                    );
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
                    {
                        if let Some(missing) = self.session.missing_sample(clip.sample_id) {
                            response.clone().on_hover_text(format!(
                                "{}\n{}\nRight-click to resolve. Import details has copyable metadata.",
                                missing_sample_details(missing),
                                missing_clip_details(clip)
                            ));
                        } else if let Some(sample) =
                            self.session.library().sample_by_id(clip.sample_id)
                        {
                            response.clone().on_hover_text(&sample.name);
                        }
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
                    if add_track.is_some() {
                        let target = Rect::from_min_size(
                            Pos2::new(rect.left(), rect.top() + occupied_count as f32 * row_height),
                            Vec2::new(width, row_height),
                        )
                        .intersect(canvas_clip);
                        if target.height() >= 22.0 {
                            painter.text(
                                target.center(),
                                Align2::CENTER_CENTER,
                                "Drop a sound here",
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
                                self.session.missing_sample(clip.sample_id),
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
                                // The erased clip may be part of the selection.
                                self.reconcile_selection();
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
                    if response.secondary_clicked() {
                        self.context_missing_sample = response.interact_pointer_pos().and_then(|pos| {
                            self.session.project().clips.iter().rev()
                                .find(|clip| self.clip_rect(clip, rect).contains(pos))
                                .and_then(|clip| self.session.missing_sample(clip.sample_id))
                                .map(|missing| missing.id)
                        });
                    }
                    if let Some(sample_id) = self.context_missing_sample {
                        response.context_menu(|ui| {
                            if ui.button("Resolve manually").clicked() {
                                ui.close();
                                self.resolve_missing_sample(sample_id);
                            }
                        });
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
                            response.widget_info(|| {
                                egui::WidgetInfo::labeled(
                                    egui::WidgetType::Button,
                                    ui.is_enabled(),
                                    format!("Add track {}", index + 1),
                                )
                            });
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
                        arm.widget_info(|| {
                            egui::WidgetInfo::selected(
                                egui::WidgetType::RadioButton,
                                ui.is_enabled(),
                                self.armed_track == index,
                                format!("Record on track {}", index + 1),
                            )
                        });
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
                            let mut response = ui.interact(
                                button,
                                ui.id().with(("route", index, left)),
                                Sense::click(),
                            );
                            let mut enabled = enabled;
                            if response.clicked() {
                                enabled = !enabled;
                                route_change = Some((index, left, enabled));
                                response.mark_changed();
                            }
                            let label = format!("Arrangement track {} {} output", index + 1, if left { "left" } else { "right" });
                            response.widget_info(|| {
                                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), enabled, &label)
                            });
                            speaker(ui.painter(), button, enabled, left);
                            if response.has_focus() {
                                ui.painter().rect_stroke(button.shrink(1.0), 2.0, Stroke::new(1.0_f32, ACCENT), egui::StrokeKind::Inside);
                            }
                            response.on_hover_text(route_hint(&label, enabled));
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
        let ruler_response = name_control(
            ui.interact(
                ruler_clip,
                ui.id().with("bar_ruler"),
                Sense::click_and_drag(),
            ),
            "Bar ruler",
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
            // The mixer is laid out before the arrangement. Redraw it with the
            // new routing state even when the pointer stops after this click.
            ui.ctx().request_repaint();
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
        if self.blocking_modal(ctx).is_none() {
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
        self.sample_conflicts_dialog(ctx);
        self.path_prompt_dialog(ctx);
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
            FontId::proportional(23.0),
            TEXT,
        );
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, &text));
    response.on_hover_text(format!("Bar {bars}, beat {beat}"))
}

pub(super) fn effects_controls(ui: &mut egui::Ui, state: &mut EffectsState) {
    egui::Frame::new().fill(controls::FACE).show(ui, |ui| {
        ui.visuals_mut().override_text_color = Some(controls::CREAM);
        ui.vertical(|ui| {
            let mut enabled = state.filter.is_some();
            if embedded_effect_group(ui, "Filter", &mut enabled) {
                state.filter = enabled.then(Default::default);
            }
            let mut filter = state.filter.clone().unwrap_or_default();
            ui.add_enabled_ui(state.filter.is_some(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    embedded_effect_dial(
                        ui,
                        &mut filter.cutoff_hz,
                        "Cutoff",
                        [20.0, 20000.0, 2000.0, 1.0],
                        0,
                        true,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut filter.resonance,
                        "Resonance",
                        [0.0, 1.0, 0.0, 0.001],
                        3,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut filter.drive,
                        "Overdrive",
                        [0.0, 1.0, 0.0, 0.001],
                        3,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut filter.wet,
                        "Filter volume",
                        [0.0, 1.0, 1.0, 0.001],
                        3,
                        false,
                    );
                });
            });
            if state.filter.is_some() {
                state.filter = Some(filter);
            }
            let mut enabled = state.echo.is_some();
            if embedded_effect_group(ui, "Echo", &mut enabled) {
                state.echo = enabled.then(Default::default);
            }
            let mut echo = state.echo.clone().unwrap_or_default();
            ui.add_enabled_ui(state.echo.is_some(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    embedded_effect_dial(
                        ui,
                        &mut echo.delay_beats,
                        "Delay (beats)",
                        [0.001, 2.0, 0.5, 0.01],
                        3,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut echo.feedback,
                        "Feedback",
                        [0.0, 0.95, 0.4, 0.01],
                        2,
                        false,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut echo.wet,
                        "Echo volume",
                        [0.0, 1.0, 0.35, 0.01],
                        2,
                        false,
                    );
                });
            });
            if state.echo.is_some() {
                state.echo = Some(echo);
            }
            let mut enabled = state.robotize.is_some();
            if embedded_effect_group(ui, "Robotize", &mut enabled) {
                state.robotize = enabled.then(Default::default);
            }
            let mut robot = state.robotize.clone().unwrap_or_default();
            ui.add_enabled_ui(state.robotize.is_some(), |ui| {
                ui.horizontal_wrapped(|ui| {
                    embedded_effect_dial(
                        ui,
                        &mut robot.frequency_hz,
                        "Color",
                        [1.0, 2000.0, 80.0, 1.0],
                        0,
                        true,
                    );
                    embedded_effect_dial(
                        ui,
                        &mut robot.wet,
                        "Robotize volume",
                        [0.0, 1.0, 0.5, 0.01],
                        2,
                        false,
                    );
                });
            });
            if state.robotize.is_some() {
                state.robotize = Some(robot);
            }
            let (band, _) =
                ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
            controls::section(ui, band, "Distortion / output");
            ui.horizontal_wrapped(|ui| {
                embedded_effect_dial(
                    ui,
                    &mut state.dirty,
                    "Dirty distortion",
                    [0.0, 1.0, 0.0, 0.001],
                    3,
                    false,
                );
                embedded_effect_dial(
                    ui,
                    &mut state.noise,
                    "Noise distortion",
                    [0.0, 1.0, 0.0, 0.001],
                    3,
                    false,
                );
                embedded_effect_dial(
                    ui,
                    &mut state.volume,
                    "Output volume",
                    [0.0, 2.0, 1.0, 0.01],
                    2,
                    false,
                );
            });
        });
    });
}

fn embedded_effect_group(ui: &mut egui::Ui, name: &str, enabled: &mut bool) -> bool {
    let (band, _) = ui.allocate_exact_size(Vec2::new(ui.available_width(), 26.0), Sense::hover());
    controls::section(ui, band, "");
    controls::toggle_at(ui, band.shrink2(Vec2::new(8.0, 0.0)), name, name, enabled)
        .on_hover_text(format!("{name}: turning it off discards its settings."))
        .changed()
}

fn embedded_effect_dial(
    ui: &mut egui::Ui,
    value: &mut f32,
    name: &str,
    settings: [f64; 4],
    decimals: usize,
    logarithmic: bool,
) {
    let [low, high, reset, step] = settings;
    let (bounds, _) = ui.allocate_exact_size(Vec2::new(126.0, 112.0), Sense::hover());
    // Absolute value fields must only advance their own column. Adding them
    // to the horizontal parent shifts the next dial down to the field baseline.
    let mut column = ui.new_child(
        egui::UiBuilder::new()
            .id_salt(name)
            .max_rect(bounds)
            .layout(egui::Layout::top_down(egui::Align::Min)),
    );
    column.set_clip_rect(bounds.intersect(ui.clip_rect()));
    let ui = &mut column;
    let mut draft = f64::from(*value);
    let dial = Rect::from_center_size(
        bounds.center_top() + Vec2::new(0.0, 29.0),
        Vec2::splat(48.0),
    );
    let response = controls::rotary(
        ui,
        dial,
        &mut draft,
        controls::RotarySpec {
            name,
            low,
            high,
            reset,
            step,
            decimals,
            logarithmic,
            choices: &[],
        },
    );
    if response.changed() {
        *value = draft as f32;
    }
    let well = Rect::from_min_size(bounds.min + Vec2::new(13.0, 61.0), Vec2::new(100.0, 24.0));
    controls::well(ui, well);
    ui.scope(|ui| {
        controls::value_style(ui);
        name_control(
            ui.put(
                well.shrink(2.0),
                egui::DragValue::new(value)
                    .range(low..=high)
                    .speed(step)
                    .fixed_decimals(decimals),
            ),
            &format!("{name} value"),
        );
    });
    ui.painter().text(
        bounds.center_top() + Vec2::new(0.0, 99.0),
        Align2::CENTER_CENTER,
        name,
        FontId::proportional(13.0),
        controls::CREAM,
    );
}

pub(super) fn pan_control(ui: &mut egui::Ui, index: usize, pan: &mut f32) -> Response {
    let response = ui.add(egui::Slider::new(pan, -1.0..=1.0).show_value(false));
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::slider(
            ui.is_enabled(),
            f64::from(*pan),
            format!("Track {} pan", index + 1),
        );
        info.current_text_value = Some(pan_description(*pan));
        info
    });
    response.on_hover_text(format!(
        "Track {} pan: {}. Pan does not enable a disabled L/R output.",
        index + 1,
        pan_description(*pan)
    ))
}

pub(super) fn pan_description(pan: f32) -> String {
    if pan == 0.0 {
        "Center".into()
    } else {
        format!(
            "{:.0}% {}",
            pan.abs() * 100.0,
            if pan < 0.0 { "left" } else { "right" }
        )
    }
}

pub(super) fn mixer_sample_summary(
    project: &Project,
    library: LibraryView<'_>,
    index: usize,
) -> (String, String) {
    let ids: BTreeSet<u32> = project
        .clips
        .iter()
        .filter(|clip| clip.track == index)
        .map(|clip| clip.sample_id)
        .collect();
    let names: Vec<String> = ids
        .iter()
        .map(|id| {
            if let Some(missing) = project
                .missing_samples
                .iter()
                .find(|sample| sample.id == *id)
            {
                return format!("Missing: {}", missing.name);
            }
            library.sample_by_id(*id).map_or_else(
                || format!("Missing sound {id}"),
                |sample| sample.name.clone(),
            )
        })
        .collect();
    let summary = match names.as_slice() {
        [] => "No sounds".into(),
        [name] => name.clone(),
        _ => format!("{} sounds", names.len()),
    };
    (summary, names.join("\n"))
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
    let response = egui::ComboBox::from_id_salt("record_destination")
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
        .response;
    name_control(response, "Recording track").on_hover_text(
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
    style
        .text_styles
        .insert(egui::TextStyle::Button, FontId::proportional(13.0));
    style
        .text_styles
        .insert(egui::TextStyle::Small, FontId::proportional(11.0));
    style
        .text_styles
        .insert(egui::TextStyle::Monospace, FontId::proportional(13.0));
    style.text_styles.insert(
        egui::TextStyle::Heading,
        FontId::new(20.0, egui::FontFamily::Name("Syncopate Bold".into())),
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

fn action_icon_button(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    label: &str,
) -> Response {
    let response = ui.add(
        egui::Button::new("")
            .fill(RAISED)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(48.0, 30.0)),
    );
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), response.rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        TEXT,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), label)
    });
    response.on_hover_text(label)
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
        icons.icon_rect(ui.ctx(), rect.center()),
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
        icons.icon_rect(ui.ctx(), rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        MUTED,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Image, ui.is_enabled(), icon.label())
    });
    response.on_hover_text(icon.label());
}

pub(super) fn category_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    // A category can have the name of a tool, such as Step synth.
    let action = format!("Show {label} sounds");
    name_control(
        ui.add(
            egui::Button::new(egui::RichText::new(label).color(if active { TEXT } else { MUTED }))
                .fill(if active { HOVER } else { PANEL })
                .stroke(Stroke::new(1.0_f32, if active { MUTED } else { PANEL }))
                .min_size(Vec2::new(40.0, 26.0)),
        ),
        &action,
    )
    .on_hover_text(action)
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

/// Width of a one-bar sound tile and the gap between tiles.
pub(super) const TILE_BAR: f32 = 96.0;
pub(super) const TILE_GAP: f32 = 8.0;

/// A tile spans one bar unit per bar plus the gaps between them, so tiles
/// of different lengths align. A sound longer than a row fills the row.
pub(super) fn tile_width(bars: u32, row_width: f32) -> f32 {
    (bars.max(1) as f32 * (TILE_BAR + TILE_GAP) - TILE_GAP).min(row_width.max(TILE_BAR))
}

/// The index of the first tile in each row when tiles wrap at `row_width`.
pub(super) fn tile_rows(widths: impl IntoIterator<Item = f32>, row_width: f32) -> Vec<usize> {
    let mut starts = Vec::new();
    let mut x = 0.0;
    for (index, width) in widths.into_iter().enumerate() {
        if index == 0 || x + width > row_width {
            starts.push(index);
            x = 0.0;
        }
        x += width + TILE_GAP;
    }
    starts
}

pub(super) fn sample_button(
    ui: &mut egui::Ui,
    sample: &Sample,
    product: Option<Product>,
    selected: bool,
    width: f32,
) -> Response {
    let (rect, response) = ui.allocate_exact_size(Vec2::new(width, 46.0), Sense::click_and_drag());
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
    painter.rect_filled(
        rect,
        5.0,
        Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), 51),
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
    let clip = painter.with_clip_rect(rect.shrink2(Vec2::new(8.0, 3.0)));
    clip.galley(rect.min + Vec2::new(8.0, 5.0), text, TEXT);
    let bars = format!(
        "{} bar{}",
        sample.bars,
        if sample.bars == 1 { "" } else { "s" }
    );
    let font = FontId::proportional(10.0);
    let muted = TEXT.gamma_multiply(0.8);
    let mut metadata = painter.layout_no_wrap(
        format!("{}  /  {bars}", sample.group_name),
        font.clone(),
        muted,
    );
    // A narrow tile keeps the bar count; its tint still names the category.
    if metadata.size().x > rect.width() - 16.0 {
        metadata = painter.layout_no_wrap(bars, font, muted);
    }
    clip.galley(
        rect.min + Vec2::new(8.0, 32.0 - metadata.size().y / 2.0),
        metadata,
        muted,
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Button, ui.is_enabled(), &sample.name)
    });
    // Build the details only for the tile under the pointer.
    response.on_hover_ui(|ui| {
        ui.label(sample_details(sample, product));
    })
}

pub(super) fn sample_details(sample: &Sample, product: Option<Product>) -> String {
    let mut rows = vec![
        sample.name.clone(),
        format!(
            "{} / {}",
            product.map_or("User WAV", Product::label),
            sample.group_name
        ),
        format!(
            "{} bar{} · {} BPM",
            sample.bars,
            if sample.bars == 1 { "" } else { "s" },
            sample.source_bpm
        ),
        format!(
            "{} Hz · {} channel{} · {} frames",
            sample.sample_rate,
            sample.channels,
            if sample.channels == 1 { "" } else { "s" },
            sample.frame_count
        ),
    ];
    if sample.sample_rate > 0 {
        rows.push(format!(
            "Audio duration: {:.3} s",
            f64::from(sample.frame_count) / f64::from(sample.sample_rate)
        ));
    }
    if !sample.code.is_empty() {
        rows.push(format!("Code: {}", sample.code));
    }
    rows.push(format!("Project sample ID: {}", sample.id));
    if let Some((_, native_id)) = Product::sample_key(sample.id) {
        rows.push(format!("Native sample ID: {native_id}"));
    }
    if let Some(path) = &sample.wav_path {
        rows.push(format!("WAV: {}", path.display()));
    }
    display_paths(&rows.join("\n"))
}

pub(super) fn draw_clip(
    painter: &egui::Painter,
    rect: Rect,
    sample: Option<&Sample>,
    missing: Option<&MissingSample>,
    selected: bool,
    gain: f32,
) {
    let color = if missing.is_some() {
        RAISED
    } else {
        sample.map(|s| group_color(&s.group_name)).unwrap_or(MUTED)
    };
    painter.rect_filled(rect, 3.0, color);
    if missing.is_some() {
        outline(painter, rect.shrink(0.5), CORAL, 1.0);
    }
    let clipped = painter.with_clip_rect(painter.clip_rect().intersect(rect.shrink(2.0)));
    if selected {
        painter.rect_stroke(
            rect.shrink(0.5),
            3.0,
            Stroke::new(2.0_f32, TEXT),
            egui::StrokeKind::Inside,
        );
    }
    let label = missing
        .map(|missing| format!("Missing: {}", missing.name))
        .or_else(|| sample.map(|sample| sample.name.clone()));
    if let Some(label) = label {
        let text_color = if missing.is_some() { TEXT } else { CANVAS };
        let size = if rect.height() < 28.0 { 10.0 } else { 12.0 };
        let text = clipped.layout(
            visible_clip_label(
                &label,
                rect.width() - 12.0,
                size,
                if rect.height() < 38.0 {
                    20.0
                } else {
                    rect.height()
                },
            ),
            FontId::proportional(size),
            text_color,
            (rect.width() - 12.0).max(1.0),
        );
        clipped.galley(rect.left_top() + Vec2::new(6.0, 4.0), text, text_color);
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

pub(super) fn missing_clip_details(clip: &Clip) -> String {
    format!(
        "Track {}: start {} ticks; length {} ticks ({:.3} bars); source offset {} frames; gain {}.",
        clip.track + 1,
        clip.start_tick,
        clip.length_ticks,
        clip.length_ticks as f64 / TICKS_PER_BAR as f64,
        clip.offset_frames,
        clip.gain
    )
}

pub(super) fn missing_sample_details(missing: &MissingSample) -> String {
    let mut rows = vec![
        format!("Missing: {}", missing.name),
        "Playback: silence".into(),
        format!("Reason: {}", missing.reason),
        format!("Project sample ID: {}", missing.id),
    ];
    match &missing.reference {
        MissingReference::Native { product, native_id } => {
            rows.push(format!("Original product: {}", product.label()));
            rows.push(format!("Native sample ID: {native_id}"));
        }
        MissingReference::External { path, origin } => {
            rows.push(format!("Original path: {}", path.display()));
            if let Some(product) = origin {
                rows.push(format!("Original product: {}", product.label()));
            }
        }
        MissingReference::Generated {
            product,
            kind,
            data,
        } => {
            rows.push(format!("Original product: {}", product.label()));
            rows.push(format!("Generator: {kind}"));
            rows.push(format!("Generator data: {data}"));
        }
    }
    rows.push(format!("Source tempo: {} BPM", missing.source_bpm));
    rows.push(format!("Trim reference rate: {} Hz", missing.sample_rate));
    rows.push(format!(
        "Source length: {} ticks ({:.3} bars; {})",
        missing.nominal_ticks,
        missing.nominal_ticks as f64 / TICKS_PER_BAR as f64,
        if missing.length_inferred {
            "inferred"
        } else {
            "known"
        }
    ));
    for (name, value) in &missing.metadata {
        rows.push(format!("{name}: {value}"));
    }
    display_paths(&rows.join("\n"))
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

fn route_hint(label: &str, active: bool) -> String {
    format!(
        "{label}: {}. Turn both outputs off to mute this track.",
        if active { "on" } else { "off" }
    )
}

fn route_color(active: bool) -> Color32 {
    if active { ACCENT } else { MUTED }
}

pub(super) fn speaker(painter: &egui::Painter, rect: Rect, enabled: bool, left: bool) {
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        if left { "L" } else { "R" },
        FontId::proportional(10.0),
        route_color(enabled),
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
    let families: [(&str, &[&[u8]]); 2] = [
        (
            "Inter Medium",
            &[include_bytes!("../../assets/fonts/Inter-Medium.ttf")],
        ),
        (
            "Syncopate Bold",
            &[include_bytes!("../../assets/fonts/Syncopate-Bold.ttf")],
        ),
    ];
    for (family, subsets) in families {
        let mut names = Vec::new();
        for (index, &bytes) in subsets.iter().enumerate() {
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
        if family == "Inter Medium" {
            fonts
                .families
                .insert(egui::FontFamily::Proportional, names.clone());
            fonts.families.insert(egui::FontFamily::Monospace, names);
        }
    }
    ctx.set_fonts(fonts);
}
