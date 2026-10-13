//! Time Stretcher body. Rendering and file operations stay in Session.
use super::*;
pub(super) struct Source<'a> {
    pub name: &'a str,
    pub id: Option<u32>,
    pub samples: &'a [Sample],
    pub missing: bool,
    pub bpm: f64,
}

pub(super) struct Feedback<'a> {
    pub processing: bool,
    pub listening: bool,
    pub saved: bool,
    pub audition: Option<(u32, f64, f64)>,
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    Select(u32),
    Listen,
    Stop,
    Save,
    Details,
    Help,
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    original: &mut f64,
    target: &mut f64,
) -> Action {
    let mut action = Action::None;
    let response = dialog::tool(
        ctx,
        "Time Stretcher",
        open,
        Vec2::new(640.0, 352.0),
        controls::Palette::Marine,
        true,
        |ui| {
            let (bounds, _) = ui.allocate_exact_size(Vec2::new(640.0, 352.0), Sense::hover());
            let rect =
                |x, y, w, h| Rect::from_min_size(bounds.min + Vec2::new(x, y), Vec2::new(w, h));
            controls::inset(ui, rect(8.0, 4.0, 624.0, 44.0));
            for x in [16.0, 332.0] {
                controls::inset(ui, rect(x, 58.0, 292.0, 146.0));
            }
            let selected = source.id;
            let mut chosen_source = None;
            at(ui, rect(16.0, 12.0, 608.0, 30.0), |ui| {
                let sounds = egui::ComboBox::from_id_salt("stretch-source")
                    .width(604.0)
                    .truncate()
                    .height(320.0)
                    .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
                    .selected_text(source.name)
                    .show_ui(ui, |ui| {
                        ui.set_width(580.0);
                        // Every opening starts with an empty, focused search.
                        let search_id = ui.id().with("source-search");
                        let seen_id = ui.id().with("source-search-seen");
                        let frame = ui.ctx().cumulative_frame_nr();
                        let opened = ui
                            .ctx()
                            .data(|data| data.get_temp::<u64>(seen_id))
                            .is_none_or(|seen| frame > seen + 1);
                        ui.ctx().data_mut(|data| data.insert_temp(seen_id, frame));
                        let mut search = if opened {
                            String::new()
                        } else {
                            ui.ctx()
                                .data(|data| data.get_temp::<String>(search_id))
                                .unwrap_or_default()
                        };
                        let search_response = ui.add(
                            egui::TextEdit::singleline(&mut search)
                                .hint_text("Find source sound")
                                .desired_width(ui.available_width()),
                        );
                        if opened {
                            search_response.request_focus();
                        }
                        name_control(search_response, "Find source sound");
                        ui.ctx()
                            .data_mut(|data| data.insert_temp(search_id, search.clone()));
                        let query = search.trim().to_lowercase();
                        let matches: Vec<_> = source
                            .samples
                            .iter()
                            .filter(|sample| {
                                query.is_empty() || sample.name.to_lowercase().contains(&query)
                            })
                            .collect();
                        if matches.is_empty() {
                            ui.label("No matching sounds. Clear the search or enter another name.");
                        } else {
                            let row_height = ui.spacing().interact_size.y.max(28.0);
                            egui::ScrollArea::vertical()
                                .id_salt(("filtered-sources", &query))
                                .max_height(260.0)
                                .show_rows(ui, row_height, matches.len(), |ui, rows| {
                                    for index in rows {
                                        let sample = matches[index];
                                        let (row, _) = ui.allocate_exact_size(
                                            Vec2::new(ui.available_width(), row_height),
                                            Sense::hover(),
                                        );
                                        if controls::button_at(
                                            ui,
                                            row,
                                            &format!("source-{}", sample.id),
                                            &sample.name,
                                            if selected == Some(sample.id) {
                                                controls::ButtonRole::Primary
                                            } else {
                                                controls::ButtonRole::Secondary
                                            },
                                            true,
                                        )
                                        .on_hover_text(&sample.name)
                                        .clicked()
                                        {
                                            chosen_source = Some(sample.id);
                                            ui.close();
                                        }
                                    }
                                });
                        }
                    })
                    .response;
                name_control(sounds, "Source sound").on_hover_text(source.name);
            });
            if let Some(id) = chosen_source {
                action = Action::Select(id);
            }
            for (x, name) in [(16.0, "Original BPM"), (332.0, "Target BPM")] {
                controls::section(ui, rect(x + 8.0, 62.0, 276.0, 24.0), name);
            }
            controls::rotary(
                ui,
                rect(136.0, 98.0, 52.0, 52.0),
                original,
                controls::RotarySpec {
                    name: "Original BPM knob",
                    low: 30.0,
                    high: 300.0,
                    reset: source.bpm,
                    step: 1.0,
                    decimals: 0,
                    logarithmic: false,
                    choices: &[],
                },
            );
            controls::rotary(
                ui,
                rect(452.0, 98.0, 52.0, 52.0),
                target,
                controls::RotarySpec {
                    name: "Target BPM knob",
                    low: 120.0,
                    high: 180.0,
                    reset: 140.0,
                    step: 20.0,
                    decimals: 0,
                    logarithmic: false,
                    choices: &SUPPORTED_BPMS,
                },
            );
            controls::well(ui, rect(104.0, 164.0, 116.0, 30.0));
            ui.scope(|ui| {
                controls::value_style(ui);
                let value = ui.put(
                    rect(110.0, 166.0, 104.0, 26.0),
                    egui::DragValue::new(original)
                        .range(30.0..=300.0)
                        .speed(1.0),
                );
                name_control(value, "Original BPM")
                    .on_hover_text("Original BPM: drag, or click to enter a value.");
            });
            at(ui, rect(420.0, 164.0, 116.0, 30.0), |ui| {
                let picker = egui::ComboBox::from_id_salt("target-picker")
                    .width(112.0)
                    .selected_text(format!("{target:.0}"))
                    .show_ui(ui, |ui| {
                        for bpm in SUPPORTED_BPMS {
                            if controls::button(
                                ui,
                                &format!("{bpm:.0} BPM"),
                                if *target == bpm {
                                    controls::ButtonRole::Primary
                                } else {
                                    controls::ButtonRole::Secondary
                                },
                            )
                            .clicked()
                            {
                                *target = bpm;
                                ui.close();
                            }
                        }
                    })
                    .response;
                name_control(picker, "Target BPM")
                    .on_hover_text("Target BPM: 120, 140, 160, or 180.");
            });
            let previewed =
                feedback
                    .audition
                    .is_some_and(|(id, audition_original, audition_target)| {
                        source.id == Some(id)
                            && *original == audition_original
                            && *target == audition_target
                    });
            let mut idle = false;
            let status = if source.missing {
                "Missing audio. Open Import details to resolve the sound.".to_owned()
            } else if let Some(message) = feedback.message {
                message.to_owned()
            } else if feedback.processing {
                "Processing sound...".to_owned()
            } else if feedback.listening {
                let (_, audition_original, audition_target) =
                    feedback.audition.unwrap_or((0, *original, *target));
                format!(
                    "Playing {audition_original:.0} to {audition_target:.0} BPM.{}",
                    if previewed {
                        ""
                    } else {
                        " Settings changed since this preview."
                    }
                )
            } else if source.id.is_none() {
                "Select a source sound to begin.".to_owned()
            } else if *original == *target {
                "Same tempo. Duration is unchanged.".to_owned()
            } else {
                idle = true;
                format!(
                    "Duration {:+.0}%. Pitch preservation is approximate.{}",
                    (*original / *target - 1.0) * 100.0,
                    if previewed {
                        ""
                    } else {
                        " Listen before saving."
                    }
                )
            };
            let enabled = source.id.is_some() && !source.missing && !feedback.processing;
            let unavailable = if feedback.processing {
                "Wait for sound processing to finish."
            } else {
                "Select an available sound to process it."
            };
            let listen_name = if feedback.listening {
                "Stop listening"
            } else {
                "Listen to stretched sound"
            };
            let listen = controls::button_at(
                ui,
                rect(16.0, 220.0, 292.0, 36.0),
                "listen",
                if feedback.listening {
                    "Stop listening"
                } else if feedback.processing {
                    "Processing..."
                } else {
                    "Listen"
                },
                controls::ButtonRole::Secondary,
                enabled || feedback.listening,
            );
            if name_control(listen, listen_name)
                .on_disabled_hover_text(unavailable)
                .clicked()
            {
                action = if feedback.listening {
                    Action::Stop
                } else {
                    Action::Listen
                };
            }
            if controls::button_at(
                ui,
                rect(332.0, 220.0, 292.0, 36.0),
                "save",
                "Save stretched sound",
                controls::ButtonRole::Primary,
                enabled,
            )
            .on_disabled_hover_text(unavailable)
            .clicked()
            {
                action = Action::Save;
            }
            if feedback.saved {
                ui.painter().text(
                    rect(332.0, 258.0, 292.0, 18.0).center(),
                    Align2::CENTER_CENTER,
                    "Saved",
                    FontId::proportional(12.0),
                    controls::CREAM,
                );
            }
            let footer_rect = rect(16.0, 280.0, 608.0, 64.0);
            controls::well(ui, footer_rect);
            let footer = ui.interact(footer_rect, ui.id().with("status-footer"), Sense::hover());
            footer.widget_info(|| {
                egui::WidgetInfo::labeled(egui::WidgetType::Panel, true, "Time Stretcher status")
            });
            let text_rect = rect(
                24.0,
                286.0,
                if source.missing { 456.0 } else { 592.0 },
                52.0,
            );
            let previous_id = ui.id().with("previous-status");
            let changed = ctx.data(|data| {
                data.get_temp::<String>(previous_id).as_deref() != Some(status.as_str())
            });
            ctx.data_mut(|data| data.insert_temp(previous_id, status.clone()));
            let mut scroll = egui::ScrollArea::vertical()
                .id_salt("stretch-status")
                .max_height(text_rect.height())
                .auto_shrink([false, false]);
            if changed {
                scroll = scroll.vertical_scroll_offset(0.0);
            }
            ui.scope_builder(egui::UiBuilder::new().max_rect(text_rect),|ui|{
            ui.set_clip_rect(text_rect.intersect(ui.clip_rect()));
            scroll.show(ui,|ui|{
                let color=if feedback.error {CORAL} else {controls::CREAM};
                let mut job=egui::text::LayoutJob::simple(status.clone(),FontId::proportional(13.0),color,ui.available_width());
                job.halign=egui::Align::Center;
                let galley=ui.fonts_mut(|fonts|fonts.layout_job(job));
                let (area,label)=ui.allocate_exact_size(Vec2::new(ui.available_width(),galley.mesh_bounds.height().max(text_rect.height())),Sense::hover());
                let origin=if galley.mesh_bounds.height()<=text_rect.height() {area.center()-galley.mesh_bounds.center().to_vec2()} else {Pos2::new(area.center().x-galley.mesh_bounds.center().x,area.top()-galley.mesh_bounds.top())};
                ui.painter().galley(origin,galley,color);
                let spoken=if feedback.error {format!("Error: {status}")} else {status.clone()};
                label.widget_info(||egui::WidgetInfo::labeled(egui::WidgetType::Label,true,&spoken));
                ctx.accesskit_node_builder(label.id,|node|node.set_live(egui::accesskit::Live::Polite));
                label.on_hover_text(if idle {format!("{status}\nThe result can change in tone or develop audible artifacts. Open Help for details.")} else {status.clone()});
            });
        });
            if source.missing
                && controls::button_at(
                    ui,
                    rect(488.0, 296.0, 128.0, 32.0),
                    "Import details",
                    "Import details",
                    controls::ButtonRole::Secondary,
                    true,
                )
                .clicked()
            {
                action = Action::Details;
            }
        },
    );
    if response.help {
        action = Action::Help;
    }
    action
}
fn at(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui)) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        add,
    );
}
