//! Shared styling and small controls: the egui style, outlines, the action
//! buttons, a sound tile's colors and details, the lane switches' hints and
//! letters, and the controls that the Mixer and the effects windows share.
use super::*;

pub(in super::super) fn effects_controls(ui: &mut egui::Ui, state: &mut EffectsState) {
    egui::Frame::new().fill(roles().face).show(ui, |ui| {
        ui.visuals_mut().override_text_color = Some(roles().legend);
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
        .hover_hint(format!("{name}: turning it off discards its settings."))
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
                well.shrink2(Vec2::new(2.0, 0.0)),
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
        roles().legend,
    );
}

/// A track's pan knob. While its pan lane has points, the knob is
/// `automated`: the lane sets the pan while the song plays. The knob leaves
/// the wheel to the view behind it, so a scroll over the strips never moves
/// a pan.
/// A pan slider named "{name} pan".
pub(in super::super) fn pan_control(
    ui: &mut egui::Ui,
    name: &str,
    pan: &mut f32,
    automated: bool,
) -> Response {
    let response = ui.add(egui::Slider::new(pan, -1.0..=1.0).show_value(false));
    response.widget_info(|| {
        let mut info =
            egui::WidgetInfo::slider(ui.is_enabled(), f64::from(*pan), format!("{name} pan"));
        info.current_text_value = Some(pan_description(*pan));
        info
    });
    response.hover_hint(format!(
        "{name} pan: {}. Drag or use the arrow keys; the wheel scrolls. Pan does not enable \
         a disabled L/R output.{}",
        pan_description(*pan),
        if automated {
            "\nAutomated: its automation lane sets the pan while the song plays."
        } else {
            ""
        }
    ))
}

pub(in super::super) fn pan_description(pan: f32) -> String {
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

/// The summary of every track, from one pass over the clips: a view that
/// shows many tracks would otherwise scan every clip once per track.
pub(in super::super) fn mixer_sample_summaries(
    project: &Project,
    library: LibraryView<'_>,
) -> Vec<(String, String)> {
    let mut ids = vec![BTreeSet::new(); project.tracks.len()];
    for clip in &project.clips {
        if let Some(track) = ids.get_mut(clip.track) {
            track.insert(clip.sample_id);
        }
    }
    ids.iter()
        .map(|track| sample_summary(project, library, track))
        .collect()
}

fn sample_summary(
    project: &Project,
    library: LibraryView<'_>,
    ids: &BTreeSet<u32>,
) -> (String, String) {
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

/// The compact tempo slider: one notch per supported tempo, value at the end.
pub(in super::super) fn bpm_picker(
    ui: &mut egui::Ui,
    name: &str,
    bpm: &mut f64,
    width: f32,
) -> Response {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 24.0), Sense::hover());
    controls::stepped_slider(ui, rect, bpm, &SUPPORTED_BPMS, name, false, None)
}

pub(in super::super) fn recording_destination(
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
    if response.gained_focus() {
        response.scroll_to_me(None);
    }
    name_control(response, "Recording track")
        .hover_hint("Choose a track for recording. Unused tracks appear only when you add a sound.")
}

/// The workspace style, menus, tooltips, and fonts of the current theme.
pub(in super::super) fn configure_style(ctx: &egui::Context) {
    theme::configure_fonts(ctx);
    let roles = roles();
    let mut style = (*ctx.style()).clone();
    style.visuals = egui::Visuals::dark();
    // egui's dark-mode curve (close to gamma 0.5) suits light text on dark
    // fields but bloats dark legends on light button faces; linear thins the
    // light text. One curve serves both in the shared glyph atlas.
    style.visuals.text_alpha_from_coverage = egui::epaint::AlphaFromCoverage::Gamma(0.7);
    style.visuals.override_text_color = Some(roles.text);
    style.visuals.panel_fill = roles.canvas;
    style.visuals.window_fill = roles.panel;
    style.visuals.window_stroke = Stroke::new(1.0_f32, roles.line);
    style.visuals.window_corner_radius = egui::CornerRadius::same(roles.corner(10.0) as u8);
    style.visuals.menu_corner_radius = egui::CornerRadius::same(roles.corner(6.0) as u8);
    style.visuals.faint_bg_color = roles.raised;
    style.visuals.extreme_bg_color = roles.canvas;
    style.visuals.selection.bg_fill = roles.selection;
    style.visuals.selection.stroke = Stroke::new(1.0_f32, roles.accent);
    style.visuals.hyperlink_color = roles.accent;
    style.visuals.warn_fg_color = roles.warning;
    style.visuals.error_fg_color = roles.coral;
    if roles.material == theme::Material::Flat {
        // Studio keeps menus and tooltips flat: an edge, no cast shadow.
        style.visuals.window_shadow = egui::epaint::Shadow::NONE;
        style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
    }
    for widget in [
        &mut style.visuals.widgets.noninteractive,
        &mut style.visuals.widgets.inactive,
        &mut style.visuals.widgets.hovered,
        &mut style.visuals.widgets.active,
        &mut style.visuals.widgets.open,
    ] {
        widget.corner_radius = egui::CornerRadius::same(roles.corner(5.0) as u8);
        widget.fg_stroke = Stroke::new(1.0_f32, roles.text);
        widget.bg_stroke = Stroke::new(1.0_f32, roles.line);
        widget.bg_fill = roles.raised;
        widget.weak_bg_fill = roles.raised;
    }
    style.visuals.widgets.hovered.bg_fill = roles.hover;
    style.visuals.widgets.hovered.weak_bg_fill = roles.hover;
    style.visuals.widgets.hovered.bg_stroke = Stroke::new(1.0_f32, roles.muted);
    style.visuals.widgets.active.bg_fill = roles.hover;
    style.visuals.widgets.active.bg_stroke = Stroke::new(1.0_f32, roles.accent);
    style.visuals.widgets.noninteractive.bg_stroke = Stroke::new(1.0_f32, roles.line);
    style.visuals.widgets.noninteractive.fg_stroke = Stroke::new(1.0_f32, roles.muted);
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
        FontId::new(20.0, theme::heading_family()),
    );
    ctx.set_style(style);
}

pub(in super::super) fn outline(painter: &egui::Painter, rect: Rect, color: Color32, width: f32) {
    let stroke = Stroke::new(width, color);
    painter.line_segment([rect.left_top(), rect.right_top()], stroke);
    painter.line_segment([rect.right_top(), rect.right_bottom()], stroke);
    painter.line_segment([rect.right_bottom(), rect.left_bottom()], stroke);
    painter.line_segment([rect.left_bottom(), rect.left_top()], stroke);
}

/// These buttons override egui's fill and stroke, so focus needs its own mark.
pub(in super::super) fn focus_outline(ui: &egui::Ui, response: &Response, color: Color32) {
    if response.has_focus() {
        ui.painter().rect_stroke(
            response.rect.shrink(1.0),
            5.0,
            Stroke::new(2.0_f32, color),
            egui::StrokeKind::Inside,
        );
    }
}

pub(in super::super) fn action_button(ui: &mut egui::Ui, label: &str, active: bool) -> Response {
    let response = ui.add(
        egui::Button::new(egui::RichText::new(label).color(if active {
            roles().on_light
        } else {
            roles().text
        }))
        .fill(if active {
            roles().accent
        } else {
            roles().raised
        })
        .stroke(Stroke::NONE)
        .min_size(Vec2::new(48.0, 30.0)),
    );
    focus_outline(
        ui,
        &response,
        if active {
            roles().on_light
        } else {
            roles().accent
        },
    );
    response
}

pub(super) fn action_icon_button(
    ui: &mut egui::Ui,
    icons: &RailIcons,
    icon: RailIcon,
    label: &str,
    enabled: bool,
    hint: &str,
    width: f32,
) -> Response {
    let response = ui.add_enabled(
        enabled,
        egui::Button::new("")
            .fill(roles().raised)
            .stroke(Stroke::NONE)
            .min_size(Vec2::new(width, 30.0)),
    );
    ui.painter().image(
        icons.texture_id(icon),
        icons.icon_rect(ui.ctx(), response.rect.center()),
        Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
        if enabled {
            roles().text
        } else {
            roles().muted.gamma_multiply(0.5)
        },
    );
    focus_outline(ui, &response, roles().accent);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, label));
    response
        .hover_hint(hint)
        .disabled_hint(format!("{hint}: nothing to {}.", label.to_lowercase()))
}

pub(in super::super) fn group_color(group: &str) -> Color32 {
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

/// A sound row's category tint over its fill.
pub(in super::super) fn tile_tint(category: Color32) -> Color32 {
    Color32::from_rgba_unmultiplied(category.r(), category.g(), category.b(), 51)
}

/// The opacity of a sound row's category and length line.
pub(in super::super) const TILE_DETAIL_ALPHA: f32 = 0.9;

pub(in super::super) fn sample_details(sample: &Sample, product: Option<Product>) -> String {
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

/// The tooltip of a track switch; the arrangement and the mixer share it.
pub(in super::super) fn switch_hint(label: &str, switch: TrackSwitch, on: bool) -> String {
    let state = if on { "on" } else { "off" };
    match switch {
        TrackSwitch::Left | TrackSwitch::Right => format!(
            "{label}: {state}. With both outputs off this track has no output. Pan stays unchanged."
        ),
        TrackSwitch::Mute => {
            format!("{label}: {state}. A muted track is silent, even when soloed.")
        }
        TrackSwitch::Solo => {
            format!("{label}: {state}. While any track is soloed, only soloed tracks play.")
        }
    }
}

fn route_color(active: bool) -> Color32 {
    if active {
        roles().accent
    } else {
        roles().muted
    }
}

pub(super) fn switch_letter(painter: &egui::Painter, rect: Rect, on: bool, switch: TrackSwitch) {
    // The bar marks a switch that is on, so the state does not rest on color.
    // It moves up in a short row so it stays inside the button.
    if on {
        let below = (rect.height() / 2.0 - 2.5).min(9.0);
        painter.rect_filled(
            Rect::from_center_size(rect.center() + Vec2::new(0.0, below), Vec2::new(9.0, 2.0)),
            1.0,
            roles().accent,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        switch.letter(),
        FontId::proportional(10.0),
        route_color(on),
    );
}
