//! Effect Studio tool window (`880x542 pt`, `Palette::Red`): Source sound bar,
//! factory preset selector, tail mode selector, shared 6-bay hardware rack,
//! transport/save actions, and live status footer.
use super::rect_in;
use super::*;
use mixjam_techno::application::effects::{
    EffectsState, TailMode, effect_presets, matching_effect_preset,
};

const WINDOW_SIZE: Vec2 = Vec2::new(880.0, 542.0);
const PAD_X: f32 = 12.0;
const INNER_W: f32 = 856.0;
const HEADER_ACCENT: Color32 = Color32::from_rgb(232, 92, 92);

pub(super) struct Source<'a> {
    pub name: &'a str,
    pub available: bool,
    pub missing: bool,
}

pub(super) struct Feedback<'a> {
    pub message: Option<&'a str>,
    pub error: bool,
}

pub(super) enum Action {
    None,
    Listen,
    Save,
    Reset,
    Details,
    Help,
}

pub(super) fn show(
    ctx: &egui::Context,
    open: &mut bool,
    source: Source<'_>,
    feedback: Feedback<'_>,
    state: &mut EffectsState,
) -> Action {
    let mut action = Action::None;
    let response = dialog::tool(
        ctx,
        "Effect Studio",
        open,
        WINDOW_SIZE,
        controls::Palette::Red,
        true,
        |ui| {
            action = draw_body(ui, &source, &feedback, state);
        },
    );
    if response.help { Action::Help } else { action }
}

fn draw_body(
    ui: &mut egui::Ui,
    source: &Source<'_>,
    feedback: &Feedback<'_>,
    state: &mut EffectsState,
) -> Action {
    let (bounds, mut panel) = super::tool_panel(ui, WINDOW_SIZE);
    draw_top_bar(&mut panel, bounds, source, state);
    effects_rack::draw_rack(
        &mut panel,
        rect_in(bounds, PAD_X, 40.0, INNER_W, 392.0),
        state,
        true,
    );
    let action = draw_actions(&mut panel, bounds, source);
    if draw_footer(&mut panel, bounds, source, feedback) {
        return Action::Details;
    }
    action
}

fn draw_top_bar(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>, state: &mut EffectsState) {
    draw_source_strip(ui, rect_in(bounds, PAD_X, 6.0, 340.0, 28.0), source);
    draw_preset_combo(ui, rect_in(bounds, 360.0, 8.0, 188.0, 24.0), state);
    let tail_rect = rect_in(bounds, 556.0, 8.0, 312.0, 24.0);
    controls::instrument::key_strip(
        ui,
        controls::instrument::KeyStrip {
            rect: tail_rect,
            vertical: false,
            name: "Tail mode",
            accent: HEADER_ACCENT,
        },
        &mut state.tail_mode,
        &[
            (
                TailMode::Trim,
                "Trim",
                "Keep the original sample length fixed.",
            ),
            (
                TailMode::WrapLoop,
                "Wrap",
                "Pre-roll seams so looping patterns wrap seamlessly.",
            ),
            (
                TailMode::RingOut,
                "Ring",
                "Extend past the sample boundary for echo and reverb decay.",
            ),
        ],
    );
}

fn draw_source_strip(ui: &mut egui::Ui, well_rect: Rect, source: &Source<'_>) {
    controls::well(ui, well_rect);
    ui.scope(|ui| {
        let text = egui::RichText::new(source.name)
            .font(FontId::proportional(13.0))
            .color(roles().legend);
        let label_rect = well_rect.shrink2(Vec2::new(10.0, 3.0));
        let label = ui.put(label_rect, egui::Label::new(text).truncate());
        named(&label, egui::WidgetType::Label, "Source sound");
        ui.ctx().accesskit_node_builder(label.id, |builder| {
            builder.set_label("Source sound");
            builder.set_value(source.name);
        });
        label.hover_hint(source.name);
    });
}

fn draw_preset_combo(ui: &mut egui::Ui, rect: Rect, state: &mut EffectsState) {
    let current = matching_effect_preset(state).unwrap_or("Custom");
    let ctx = ui.ctx().clone();
    let mut preset_ui = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    let combo = egui::ComboBox::from_id_salt("effect_studio_preset")
        .selected_text(current)
        .width(rect.width())
        .height(360.0);
    let response = view::tall_menu(&ctx, || {
        combo.show_ui(&mut preset_ui, |ui| {
            for preset in effect_presets() {
                if ui
                    .selectable_label(preset.name == current, preset.name)
                    .clicked()
                {
                    *state = preset.state.clone();
                    ui.close();
                }
            }
        })
    })
    .response;
    form::choice_named(&response, "Effect preset", current);
    response.hover_hint("Load an Effect Studio preset across all six bays.");
}

fn draw_actions(ui: &mut egui::Ui, bounds: Rect, source: &Source<'_>) -> Action {
    let available = source.available && !source.missing;
    let mut action = Action::None;
    for (x, width, text, next, enabled) in [
        (PAD_X, 277.0, "Listen", Action::Listen, available),
        (301.0, 278.0, "Save sound", Action::Save, available),
        (591.0, 277.0, "Reset", Action::Reset, true),
    ] {
        if action_button(ui, rect_in(bounds, x, 442.0, width, 34.0), text, enabled).clicked() {
            action = next;
        }
    }
    action
}

fn action_button(ui: &mut egui::Ui, rect: Rect, text: &str, enabled: bool) -> Response {
    controls::button_at(
        ui,
        rect,
        text,
        text,
        if text == "Save sound" {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        enabled,
    )
    .disabled_hint("Select an available sound to process it.")
    .hover_hint(if text == "Save sound" {
        "Save a new WAV and add it to Effect."
    } else {
        text
    })
}

fn draw_footer(
    ui: &mut egui::Ui,
    bounds: Rect,
    source: &Source<'_>,
    feedback: &Feedback<'_>,
) -> bool {
    let status = if source.missing {
        "Missing audio. Open Import details to resolve the sound."
    } else if !source.available {
        "Select a sound in the palette to begin."
    } else {
        feedback
            .message
            .unwrap_or("Listen before saving. Save sound adds a new WAV to Effect.")
    };
    let status_rect = rect_in(bounds, PAD_X, 484.0, INNER_W, 42.0);
    status_footer(
        ui,
        status_rect,
        status,
        feedback.error || source.missing,
        source.missing,
    );
    source.missing
        && controls::button_at(
            ui,
            rect_in(bounds, 732.0, 490.0, 128.0, 30.0),
            "Import details",
            "Import details",
            controls::ButtonRole::Secondary,
            true,
        )
        .clicked()
}

fn status_footer(ui: &mut egui::Ui, rect: Rect, text: &str, error: bool, missing: bool) {
    controls::status_well(ui, rect, error);
    let response = ui.interact(rect, ui.id().with("status"), Sense::hover());
    named(&response, egui::WidgetType::Panel, "Effect Studio status");
    let text_rect = Rect::from_min_max(
        rect.min + Vec2::new(10.0, 6.0),
        rect.max - Vec2::new(if missing { 148.0 } else { 10.0 }, 6.0),
    );
    controls::status_text(ui, text_rect, "effects-status", text, 12.5, error, text);
}
