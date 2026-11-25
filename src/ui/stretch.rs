//! Time Stretcher body. Rendering and file operations stay in Session.
use super::rect_in;
use super::*;

/// The original tempos that the Time Stretcher offers, in BPM: a view range
/// inside the engine's `TEMPO_RANGE`.
const STRETCH_BPM: std::ops::RangeInclusive<f64> = 30.0..=300.0;
const WINDOW_SIZE: Vec2 = Vec2::new(640.0, 356.0);
const PAD_X: f32 = 12.0;
const INNER_W: f32 = 616.0;
const BAY_W: f32 = 303.0;
const LEFT_ACCENT: Color32 = Color32::from_rgb(96, 206, 232);
const RIGHT_ACCENT: Color32 = Color32::from_rgb(238, 168, 72);

/// A tempo held to the Time Stretcher's range.
pub(super) fn clamp_bpm(bpm: f64) -> f64 {
    bpm.clamp(*STRETCH_BPM.start(), *STRETCH_BPM.end())
}
pub(super) struct Source<'a> {
    pub name: &'a str,
    pub id: Option<u32>,
    pub samples: &'a [Sample],
    pub catalog_revision: u64,
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
        WINDOW_SIZE,
        controls::Palette::Marine,
        true,
        |ui| {
            let (bounds, _) = ui.allocate_exact_size(WINDOW_SIZE, Sense::hover());
            if let Some(id) = draw_source_selector(ui, bounds, &source) {
                action = Action::Select(id);
            }
            draw_original_bay(ui, bounds, &source, original);
            draw_target_bay(ui, bounds, &feedback, *original, target);
            let previewed =
                feedback
                    .audition
                    .is_some_and(|(id, audition_original, audition_target)| {
                        source.id == Some(id)
                            && *original == audition_original
                            && *target == audition_target
                    });
            let (status, idle) = status_message(&source, &feedback, *original, *target, previewed);
            if let Some(next) = draw_action_row(ui, bounds, &source, &feedback) {
                action = next;
            }
            if draw_status_footer(ui, bounds, &source, &feedback, &status, idle) {
                action = Action::Details;
            }
        },
    );
    if response.help {
        return Action::Help;
    }
    action
}

fn draw_source_selector(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>) -> Option<u32> {
    controls::well(ui, rect_in(bounds, PAD_X, 6.0, INNER_W, 40.0));
    let mut chosen_source = None;
    at(ui, rect_in(bounds, 18.0, 11.0, 604.0, 30.0), |ui| {
        let sounds = egui::ComboBox::from_id_salt("stretch-source")
            .width(600.0)
            .truncate()
            .height(320.0)
            .close_behavior(egui::PopupCloseBehavior::CloseOnClickOutside)
            .selected_text(source.name)
            .show_ui(ui, |ui| {
                ui.set_width(580.0);
                let query = source_search_query(ui);
                let indices = matching_source_indices(ui, source, &query);
                let matches: Vec<_> = indices
                    .iter()
                    .map(|&index| &source.samples[index])
                    .collect();
                chosen_source = draw_source_rows(ui, &query, &matches, source.id);
            })
            .response;
        name_control(sounds, "Source sound").hover_hint(source.name);
    });
    chosen_source
}

fn source_search_query(ui: &mut egui::Ui) -> String {
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
    search.trim().to_lowercase()
}

fn matching_source_indices(ui: &egui::Ui, source: &Source<'_>, query: &str) -> Vec<usize> {
    let matches_id = ui.id().with("source-matches");
    let count = source.samples.len();
    let cached = ui
        .ctx()
        .data(|data| data.get_temp::<(String, u64, usize, Vec<usize>)>(matches_id))
        .filter(|(seen, revision, seen_count, _)| {
            seen == query && *revision == source.catalog_revision && *seen_count == count
        });
    if let Some((_, _, _, indices)) = cached {
        return indices;
    }
    let indices = compute_matching_indices(source.samples, query);
    ui.ctx().data_mut(|data| {
        data.insert_temp(
            matches_id,
            (
                query.to_owned(),
                source.catalog_revision,
                count,
                indices.clone(),
            ),
        );
    });
    indices
}

fn compute_matching_indices(samples: &[Sample], query: &str) -> Vec<usize> {
    samples
        .iter()
        .enumerate()
        .filter(|(_, sample)| query.is_empty() || sample.name.to_lowercase().contains(query))
        .map(|(index, _)| index)
        .collect()
}

fn draw_source_rows(
    ui: &mut egui::Ui,
    query: &str,
    matches: &[&Sample],
    selected: Option<u32>,
) -> Option<u32> {
    if matches.is_empty() {
        ui.label("No matching sounds. Clear the search or enter another name.");
        return None;
    }
    let mut chosen = None;
    let row_height = ui.spacing().interact_size.y.max(28.0);
    egui::ScrollArea::vertical()
        .id_salt(("filtered-sources", query))
        .max_height(260.0)
        .show_rows(ui, row_height, matches.len(), |ui, rows| {
            for index in rows {
                let sample = matches[index];
                let (row, _) = ui.allocate_exact_size(
                    Vec2::new(ui.available_width(), row_height),
                    Sense::hover(),
                );
                let role = if selected == Some(sample.id) {
                    controls::ButtonRole::Primary
                } else {
                    controls::ButtonRole::Secondary
                };
                if controls::button_at(
                    ui,
                    row,
                    &format!("source-{}", sample.id),
                    &sample.name,
                    role,
                    true,
                )
                .hover_hint(&sample.name)
                .clicked()
                {
                    chosen = Some(sample.id);
                    ui.close();
                }
            }
        });
    view::name_scroll_area(ui, ("filtered-sources", query), "Source sounds");
    chosen
}

fn draw_original_bay(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>, original: &mut f64) {
    let bay = rect_in(bounds, PAD_X, 56.0, BAY_W, 152.0);
    controls::instrument::bay(ui, bay, "Original BPM", LEFT_ACCENT, false);
    theme::with_knob_style(theme::KnobStyle::Anodized([56, 172, 216]), || {
        controls::rotary(
            ui,
            rect_in(bounds, 137.5, 88.0, 52.0, 52.0),
            original,
            controls::RotarySpec {
                name: "Original BPM knob",
                low: *STRETCH_BPM.start(),
                high: *STRETCH_BPM.end(),
                reset: source.bpm,
                step: 1.0,
                decimals: 0,
                logarithmic: false,
                choices: &[],
            },
        );
    });
    controls::well(ui, rect_in(bounds, 105.5, 160.0, 116.0, 30.0));
    ui.scope(|ui| {
        controls::value_style(ui);
        let value = ui.put(
            rect_in(bounds, 111.5, 162.0, 104.0, 26.0),
            egui::DragValue::new(original).range(STRETCH_BPM).speed(1.0),
        );
        name_control(value, "Original BPM")
            .hover_hint("Original BPM: drag, or click to enter a value.");
    });
}

fn draw_target_bay(
    ui: &mut egui::Ui,
    bounds: Rect,
    feedback: &Feedback<'_>,
    original: f64,
    target: &mut f64,
) {
    let bay = rect_in(bounds, 325.0, 56.0, BAY_W, 152.0);
    controls::instrument::bay(ui, bay, "Target BPM", RIGHT_ACCENT, false);
    if feedback.saved {
        let badge = rect_in(bounds, 552.0, 60.0, 58.0, 14.0);
        ui.painter().rect_filled(badge, 3.0, roles().ink);
        ui.painter().rect_stroke(
            badge,
            3.0,
            Stroke::new(1.0_f32, roles().lamp_good.gamma_multiply(0.85)),
            egui::StrokeKind::Inside,
        );
        ui.painter().circle_filled(
            Pos2::new(badge.left() + 7.5, badge.center().y),
            2.3,
            roles().lamp_good,
        );
        ui.painter().text(
            Pos2::new(badge.left() + 14.0, badge.center().y),
            Align2::LEFT_CENTER,
            "Saved",
            FontId::proportional(10.5),
            roles().legend,
        );
    }
    name_control(
        controls::stepped_slider(
            ui,
            rect_in(bounds, 345.0, 96.0, 263.0, 40.0),
            target,
            &SUPPORTED_BPMS,
            "Target BPM",
            true,
            Some(140.0),
        ),
        "Target BPM",
    );
    let ratio_well = rect_in(bounds, 345.0, 160.0, 263.0, 30.0);
    controls::well(ui, ratio_well);
    let mid_x = ratio_well.center().x;
    ui.painter().line_segment(
        [
            Pos2::new(mid_x, ratio_well.top() + 5.0),
            Pos2::new(mid_x, ratio_well.bottom() - 5.0),
        ],
        Stroke::new(1.0_f32, roles().well_edge.gamma_multiply(0.75)),
    );
    let ratio = original / *target;
    let delta_pct = (ratio - 1.0) * 100.0;
    let changed = (original - *target).abs() >= 0.05;
    let length_text = if changed {
        format!("{delta_pct:+.0}% length")
    } else {
        "0% length".to_owned()
    };
    let length_color = if changed {
        Color32::from_rgb(244, 198, 118)
    } else {
        roles().legend
    };
    ui.painter().text(
        Pos2::new(
            ratio_well.left() + ratio_well.width() * 0.25,
            ratio_well.center().y,
        ),
        Align2::CENTER_CENTER,
        format!("Ratio {ratio:.2}x"),
        FontId::proportional(12.0),
        roles().legend,
    );
    ui.painter().text(
        Pos2::new(
            ratio_well.left() + ratio_well.width() * 0.75,
            ratio_well.center().y,
        ),
        Align2::CENTER_CENTER,
        length_text,
        FontId::proportional(12.0),
        length_color,
    );
}

fn status_message(
    source: &Source<'_>,
    feedback: &Feedback<'_>,
    original: f64,
    target: f64,
    previewed: bool,
) -> (String, bool) {
    if source.missing {
        return (
            "Missing audio. Open Import details to resolve the sound.".to_owned(),
            false,
        );
    }
    if let Some(message) = feedback.message {
        return (message.to_owned(), false);
    }
    if feedback.processing {
        return ("Processing sound...".to_owned(), false);
    }
    if feedback.listening {
        let (_, audition_original, audition_target) =
            feedback.audition.unwrap_or((0, original, target));
        return (
            format!(
                "Playing {audition_original:.0} to {audition_target:.0} BPM.{}",
                if previewed {
                    ""
                } else {
                    " Settings changed since this preview."
                }
            ),
            false,
        );
    }
    if source.id.is_none() {
        return ("Select a source sound to begin.".to_owned(), false);
    }
    if original == target {
        return ("Same tempo. Duration is unchanged.".to_owned(), false);
    }
    (
        format!(
            "Duration {:+.0}%. Pitch preservation is approximate.{}",
            (original / target - 1.0) * 100.0,
            if previewed {
                ""
            } else {
                " Listen before saving."
            }
        ),
        true,
    )
}

fn listen_button_copy(feedback: &Feedback<'_>) -> (&'static str, &'static str) {
    match (feedback.listening, feedback.processing) {
        (true, _) => ("Stop listening", "Stop listening"),
        (false, true) => ("Listen to stretched sound", "Processing..."),
        (false, false) => ("Listen to stretched sound", "Listen"),
    }
}

fn draw_action_row(
    ui: &mut egui::Ui,
    bounds: Rect,
    source: &Source<'_>,
    feedback: &Feedback<'_>,
) -> Option<Action> {
    let enabled = source.id.is_some() && !source.missing && !feedback.processing;
    let unavailable = if feedback.processing {
        "Wait for sound processing to finish."
    } else {
        "Select an available sound to process it."
    };
    let (listen_name, listen_text) = listen_button_copy(feedback);
    let listen = controls::button_at(
        ui,
        rect_in(bounds, PAD_X, 220.0, BAY_W, 36.0),
        "listen",
        listen_text,
        controls::ButtonRole::Secondary,
        enabled || feedback.listening,
    );
    let listen_clicked = name_control(listen, listen_name)
        .disabled_hint(unavailable)
        .clicked();
    // Draw Save in every frame, also in the frame of a Listen click.
    let save_clicked = controls::button_at(
        ui,
        rect_in(bounds, 325.0, 220.0, BAY_W, 36.0),
        "save",
        "Save stretched sound",
        controls::ButtonRole::Primary,
        enabled,
    )
    .disabled_hint(unavailable)
    .clicked();
    if save_clicked {
        Some(Action::Save)
    } else if listen_clicked {
        Some(if feedback.listening {
            Action::Stop
        } else {
            Action::Listen
        })
    } else {
        None
    }
}

fn draw_status_footer(
    ui: &mut egui::Ui,
    bounds: Rect,
    source: &Source<'_>,
    feedback: &Feedback<'_>,
    status: &str,
    idle: bool,
) -> bool {
    let footer_rect = rect_in(bounds, PAD_X, 272.0, INNER_W, 72.0);
    controls::status_well(ui, footer_rect, feedback.error || source.missing);
    let footer = ui.interact(footer_rect, ui.id().with("status-footer"), Sense::hover());
    footer.widget_info(|| {
        egui::WidgetInfo::labeled(egui::WidgetType::Panel, true, "Time Stretcher status")
    });
    let text_rect = rect_in(
        bounds,
        22.0,
        278.0,
        if source.missing { 460.0 } else { 596.0 },
        60.0,
    );
    let hover = if idle {
        format!(
            "{status}\nThe result can change in tone or develop audible artifacts. Open Help for details."
        )
    } else {
        status.to_owned()
    };
    controls::status_text(
        ui,
        text_rect,
        "stretch-status",
        status,
        13.0,
        feedback.error,
        &hover,
    );
    source.missing
        && controls::button_at(
            ui,
            rect_in(bounds, 490.0, 292.0, 128.0, 32.0),
            "Import details",
            "Import details",
            controls::ButtonRole::Secondary,
            true,
        )
        .clicked()
}

fn at(ui: &mut egui::Ui, rect: Rect, add: impl FnOnce(&mut egui::Ui)) {
    ui.scope_builder(
        egui::UiBuilder::new()
            .max_rect(rect)
            .layout(egui::Layout::left_to_right(egui::Align::Center)),
        add,
    );
}
