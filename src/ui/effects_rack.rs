//! Unified 6-bay, 3-theme hardware rack shared by Effect Studio (`Palette::Red`)
//! and Step Synth's Effects pane (`Palette::Olive`).
use super::*;
use mixjam_techno::application::effects::{
    DistortionShape, Echo, EffectsState, Filter, FilterMode, MIN_ECHO_BEATS, Modulation,
    ModulationMode, Reverb, ReverbSpace, Robotize, RobotizeMode, sample_drive_curve,
    sample_filter_curve,
};

const GAP: f32 = 8.0;
const SCOPES_H: f32 = 72.0;
const BAY_H: f32 = 152.0;
const WIDE_THRESHOLD: f32 = 820.0;

const FILTER_ACCENT: Color32 = Color32::from_rgb(86, 204, 188);
const MOD_ACCENT: Color32 = Color32::from_rgb(122, 198, 126);
const ECHO_ACCENT: Color32 = Color32::from_rgb(96, 194, 242);
const ROBOT_ACCENT: Color32 = Color32::from_rgb(186, 152, 246);
const REVERB_ACCENT: Color32 = Color32::from_rgb(112, 168, 244);
const DISTORT_ACCENT: Color32 = Color32::from_rgb(246, 148, 76);

#[derive(Clone, Copy)]
struct DialSpec<'a> {
    name: &'a str,
    caption: &'a str,
    low: f32,
    high: f32,
    reset: f32,
    step: f32,
    decimals: usize,
    logarithmic: bool,
    suffix: &'a str,
    well_w: f32,
}

/// Height consumed by `draw_rack_scrolling` at `width`.
pub(super) fn rack_height(width: f32, show_scopes: bool) -> f32 {
    let scopes = if show_scopes { SCOPES_H + GAP } else { 0.0 };
    let rows = if width >= WIDE_THRESHOLD { 2.0 } else { 3.0 };
    scopes + rows * BAY_H + (rows - 1.0) * GAP
}

/// Render the full 6-bay effects rack into `rect`.
pub(super) fn draw_rack(
    ui: &mut egui::Ui,
    rect: Rect,
    state: &mut EffectsState,
    show_scopes: bool,
) {
    let mut y = rect.top();
    if show_scopes {
        let scopes_rect = Rect::from_min_size(rect.min, Vec2::new(rect.width(), SCOPES_H));
        draw_scopes(ui, scopes_rect, state);
        y += SCOPES_H + GAP;
    }
    let bays_rect = Rect::from_min_max(Pos2::new(rect.left(), y), rect.max);
    let rects = layout_bays(bays_rect);
    draw_filter_bay(ui, rects[0], state);
    draw_mod_bay(ui, rects[1], state);
    draw_echo_bay(ui, rects[2], state);
    draw_robotize_bay(ui, rects[3], state);
    draw_reverb_bay(ui, rects[4], state);
    draw_distortion_bay(ui, rects[5], state);
}

/// Render the 6-bay effects rack inside a vertically scrolling container.
pub(super) fn draw_rack_scrolling(ui: &mut egui::Ui, state: &mut EffectsState, show_scopes: bool) {
    let w = ui.available_width().max(520.0);
    let h = rack_height(w, show_scopes);
    let (rect, _) = ui.allocate_exact_size(Vec2::new(w, h), Sense::hover());
    let mut rack = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    draw_rack(&mut rack, rect, state, show_scopes);
}

fn draw_scopes(ui: &mut egui::Ui, rect: Rect, state: &EffectsState) {
    let half_w = (rect.width() - GAP) * 0.5;
    let left = Rect::from_min_size(rect.min, Vec2::new(half_w, rect.height()));
    let right = Rect::from_min_size(
        Pos2::new(left.right() + GAP, rect.top()),
        Vec2::new(rect.right() - left.right() - GAP, rect.height()),
    );
    let filter_curve = sample_filter_curve(&state.filter, 64);
    controls::instrument::eq_response_scope(
        ui,
        left,
        "Filter Curve",
        &filter_curve,
        24.0,
        state.filter.enabled,
        FILTER_ACCENT,
    );
    let drive_curve = sample_drive_curve(state, 65);
    let drive_active = state.dirty > 0.0 || (state.volume - 1.0).abs() > 1e-4;
    controls::instrument::scope_frame_state(
        ui,
        right,
        "Drive Curve",
        &drive_curve,
        controls::instrument::ScopeState {
            active: drive_active,
            status: (!drive_active).then_some("OFF"),
            accent: DISTORT_ACCENT,
        },
    );
}

fn row_rects(top: f32, left: f32, total_w: f32, weights: &[f32]) -> Vec<Rect> {
    let usable = total_w - GAP * (weights.len().saturating_sub(1) as f32);
    let mut x = left;
    weights
        .iter()
        .enumerate()
        .map(|(i, &w)| {
            let width = if i + 1 == weights.len() {
                (left + total_w - x).max(1.0)
            } else {
                (usable * w).round()
            };
            let rect = Rect::from_min_size(Pos2::new(x, top), Vec2::new(width, BAY_H));
            x += width + GAP;
            rect
        })
        .collect()
}

fn layout_bays(area: Rect) -> [Rect; 6] {
    if area.width() >= WIDE_THRESHOLD {
        // 2x3 horizontal signal chain: Filter | Modulation | Echo / Robotize | Reverb | Dist/Out.
        let r1 = row_rects(area.top(), area.left(), area.width(), &[0.35, 0.30, 0.35]);
        let r2 = row_rects(
            area.top() + BAY_H + GAP,
            area.left(),
            area.width(),
            &[0.243, 0.324, 0.433],
        );
        return [r1[0], r1[1], r1[2], r2[0], r2[1], r2[2]];
    }
    // 3x2 narrow responsive layout keeps Filter's 4 dials on a single horizontal row.
    let r1 = row_rects(area.top(), area.left(), area.width(), &[0.54, 0.46]);
    let r2 = row_rects(
        area.top() + BAY_H + GAP,
        area.left(),
        area.width(),
        &[0.60, 0.40],
    );
    let r3 = row_rects(
        area.top() + 2.0 * (BAY_H + GAP),
        area.left(),
        area.width(),
        &[0.46, 0.54],
    );
    [r1[0], r1[1], r2[0], r2[1], r3[0], r3[1]]
}

fn switchable_bay_header(
    ui: &mut egui::Ui,
    rect: Rect,
    title: &str,
    accent: Color32,
    enabled: &mut bool,
    hint: &str,
) -> Rect {
    controls::instrument::bay_with_state(ui, rect, title, accent, false, *enabled);
    let toggle_rect = Rect::from_min_size(rect.min + Vec2::new(10.0, 24.0), Vec2::new(28.0, 20.0));
    controls::instrument::bat_toggle(ui, toggle_rect, title, "", enabled, hint, false);
    Rect::from_min_max(
        Pos2::new(toggle_rect.right() + 8.0, toggle_rect.top()),
        Pos2::new(rect.right() - 10.0, toggle_rect.bottom()),
    )
}

fn draw_dial_row(
    ui: &mut egui::Ui,
    bay: Rect,
    enabled: bool,
    mut items: Vec<(DialSpec<'_>, &mut f32)>,
) {
    let count = items.len().max(1) as f32;
    let inner_left = bay.left() + 8.0;
    let inner_w = (bay.width() - 16.0).max(40.0);
    let slot_w = inner_w / count;
    let knob_y = bay.top() + 72.0;
    let well_y = bay.top() + 108.0;
    let label_y = bay.top() + 131.0;

    for (i, (spec, value)) in items.iter_mut().enumerate() {
        let cx = inner_left + slot_w * (i as f32 + 0.5);
        draw_single_dial(
            ui,
            cx,
            slot_w,
            [knob_y, well_y, label_y],
            *spec,
            value,
            enabled,
        );
    }
}

fn draw_single_dial(
    ui: &mut egui::Ui,
    cx: f32,
    slot_w: f32,
    ys: [f32; 3],
    spec: DialSpec<'_>,
    value: &mut f32,
    enabled: bool,
) {
    let [knob_y, well_y, label_y] = ys;
    let well_w = spec.well_w.min((slot_w - 6.0).max(40.0));
    let knob_center = Pos2::new(cx, knob_y);
    let well_center = Pos2::new(cx, well_y);
    let label_pos = Pos2::new(cx, label_y);

    ui.painter().text(
        label_pos,
        Align2::CENTER_CENTER,
        spec.caption,
        FontId::proportional(11.0),
        roles().legend,
    );
    controls::well(
        ui,
        Rect::from_center_size(well_center, Vec2::new(well_w, 20.0)),
    );
    ui.add_enabled_ui(enabled, |ui| {
        let mut draft = f64::from(*value);
        let before = draft;
        let knob = controls::rotary(
            ui,
            Rect::from_center_size(knob_center, Vec2::splat(40.0)),
            &mut draft,
            controls::RotarySpec {
                name: spec.name,
                low: f64::from(spec.low),
                high: f64::from(spec.high),
                reset: f64::from(spec.reset),
                step: f64::from(spec.step),
                decimals: spec.decimals,
                logarithmic: spec.logarithmic,
                choices: &[],
            },
        );
        controls::reveal_focus(ui, &knob);
        if draft != before {
            *value = draft as f32;
        }
        ui.scope(|ui| {
            controls::value_style(ui);
            let response = ui.put(
                Rect::from_center_size(well_center, Vec2::new(well_w - 2.0, 22.0)),
                egui::DragValue::new(value)
                    .range(spec.low..=spec.high)
                    .clamp_existing_to_range(false)
                    .speed(spec.step)
                    .fixed_decimals(spec.decimals)
                    .suffix(spec.suffix),
            );
            controls::reveal_focus(ui, &response);
            name_control(response, &format!("{} value", spec.name))
                .hover_hint(format!("{}: drag, or click to enter a value.", spec.name));
        });
    });
}

fn draw_filter_bay(ui: &mut egui::Ui, rect: Rect, state: &mut EffectsState) {
    let strip_rect = switchable_bay_header(
        ui,
        rect,
        "Filter",
        FILTER_ACCENT,
        &mut state.filter.enabled,
        "Turn the Filter section on or off.",
    );
    ui.add_enabled_ui(state.filter.enabled, |ui| {
        controls::instrument::key_strip(
            ui,
            controls::instrument::KeyStrip {
                rect: strip_rect,
                vertical: false,
                name: "Filter mode",
                accent: FILTER_ACCENT,
            },
            &mut state.filter.mode,
            &[
                (FilterMode::Lp12, "LP12", "12 dB/oct low-pass filter."),
                (
                    FilterMode::Lp24,
                    "LP24",
                    "24 dB/oct nonlinear ladder low-pass.",
                ),
                (
                    FilterMode::Bp12,
                    "BP12",
                    "12 dB/oct resonant band-pass filter.",
                ),
                (FilterMode::Hp12, "HP12", "12 dB/oct high-pass filter."),
            ],
        );
    });
    draw_dial_row(
        ui,
        rect,
        state.filter.enabled,
        vec![
            (
                DialSpec {
                    name: "Cutoff",
                    caption: "Cutoff",
                    low: 20.0,
                    high: 20000.0,
                    reset: Filter::default().cutoff_hz,
                    step: 1.0,
                    decimals: 0,
                    logarithmic: true,
                    suffix: " Hz",
                    well_w: 66.0,
                },
                &mut state.filter.cutoff_hz,
            ),
            (
                DialSpec {
                    name: "Resonance",
                    caption: "Resonance",
                    low: 0.0,
                    high: 1.0,
                    reset: Filter::default().resonance,
                    step: 0.001,
                    decimals: 3,
                    logarithmic: false,
                    suffix: "",
                    well_w: 52.0,
                },
                &mut state.filter.resonance,
            ),
            (
                DialSpec {
                    name: "Overdrive",
                    caption: "Overdrive",
                    low: 0.0,
                    high: 1.0,
                    reset: Filter::default().drive,
                    step: 0.001,
                    decimals: 3,
                    logarithmic: false,
                    suffix: "",
                    well_w: 52.0,
                },
                &mut state.filter.drive,
            ),
            (
                DialSpec {
                    name: "Filter volume",
                    caption: "Filter volume",
                    low: 0.0,
                    high: 1.0,
                    reset: Filter::default().wet,
                    step: 0.001,
                    decimals: 3,
                    logarithmic: false,
                    suffix: "",
                    well_w: 52.0,
                },
                &mut state.filter.wet,
            ),
        ],
    );
}

fn draw_mod_bay(ui: &mut egui::Ui, rect: Rect, state: &mut EffectsState) {
    let strip_rect = switchable_bay_header(
        ui,
        rect,
        "Modulation",
        MOD_ACCENT,
        &mut state.modulation.enabled,
        "Turn stereo Chorus or Phaser modulation on or off.",
    );
    ui.add_enabled_ui(state.modulation.enabled, |ui| {
        controls::instrument::key_strip(
            ui,
            controls::instrument::KeyStrip {
                rect: strip_rect,
                vertical: false,
                name: "Modulation mode",
                accent: MOD_ACCENT,
            },
            &mut state.modulation.mode,
            &[
                (
                    ModulationMode::Chorus,
                    "Chorus",
                    "Stereo quadrature BBD chorus.",
                ),
                (
                    ModulationMode::Phaser,
                    "Phaser",
                    "6-stage stereo all-pass phaser.",
                ),
            ],
        );
    });
    draw_dial_row(
        ui,
        rect,
        state.modulation.enabled,
        vec![
            (
                DialSpec {
                    name: "Modulation rate",
                    caption: "Rate",
                    low: 0.05,
                    high: 8.0,
                    reset: Modulation::default().rate_hz,
                    step: 0.05,
                    decimals: 2,
                    logarithmic: true,
                    suffix: " Hz",
                    well_w: 56.0,
                },
                &mut state.modulation.rate_hz,
            ),
            (
                DialSpec {
                    name: "Modulation depth",
                    caption: "Depth",
                    low: 0.0,
                    high: 1.0,
                    reset: Modulation::default().depth,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 48.0,
                },
                &mut state.modulation.depth,
            ),
            (
                DialSpec {
                    name: "Modulation feedback",
                    caption: "Regen",
                    low: -0.85,
                    high: 0.85,
                    reset: Modulation::default().feedback,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 50.0,
                },
                &mut state.modulation.feedback,
            ),
            (
                DialSpec {
                    name: "Modulation volume",
                    caption: "Wet",
                    low: 0.0,
                    high: 1.0,
                    reset: Modulation::default().wet,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 48.0,
                },
                &mut state.modulation.wet,
            ),
        ],
    );
}

fn draw_echo_bay(ui: &mut egui::Ui, rect: Rect, state: &mut EffectsState) {
    let strip_rect = switchable_bay_header(
        ui,
        rect,
        "Echo",
        ECHO_ACCENT,
        &mut state.echo.enabled,
        "Turn Echo on or off.",
    );
    ui.add_enabled_ui(state.echo.enabled, |ui| {
        let ping_rect = Rect::from_min_size(
            Pos2::new(
                (strip_rect.right() - 96.0).max(strip_rect.left()),
                strip_rect.top(),
            ),
            Vec2::new(96.0_f32.min(strip_rect.width()), strip_rect.height()),
        );
        controls::instrument::bat_toggle(
            ui,
            ping_rect,
            "Echo ping-pong",
            "Ping-pong",
            &mut state.echo.ping_pong,
            "Alternate repeats across left and right stereo channels.",
            false,
        );
    });
    draw_dial_row(
        ui,
        rect,
        state.echo.enabled,
        vec![
            (
                DialSpec {
                    name: "Delay (beats)",
                    caption: "Delay (beats)",
                    low: MIN_ECHO_BEATS,
                    high: 2.0,
                    reset: Echo::default().delay_beats,
                    step: 0.01,
                    decimals: 3,
                    logarithmic: false,
                    suffix: "",
                    well_w: 58.0,
                },
                &mut state.echo.delay_beats,
            ),
            (
                DialSpec {
                    name: "Feedback",
                    caption: "Feedback",
                    low: 0.0,
                    high: 0.95,
                    reset: Echo::default().feedback,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 50.0,
                },
                &mut state.echo.feedback,
            ),
            (
                DialSpec {
                    name: "Echo tone",
                    caption: "Tone",
                    low: 0.0,
                    high: 1.0,
                    reset: Echo::default().tone,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 48.0,
                },
                &mut state.echo.tone,
            ),
            (
                DialSpec {
                    name: "Echo volume",
                    caption: "Echo volume",
                    low: 0.0,
                    high: 1.0,
                    reset: Echo::default().wet,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 50.0,
                },
                &mut state.echo.wet,
            ),
        ],
    );
}

fn draw_robotize_bay(ui: &mut egui::Ui, rect: Rect, state: &mut EffectsState) {
    let strip_rect = switchable_bay_header(
        ui,
        rect,
        "Robotize",
        ROBOT_ACCENT,
        &mut state.robotize.enabled,
        "Turn Robotize Ring Modulator or Frequency Shifter on or off.",
    );
    ui.add_enabled_ui(state.robotize.enabled, |ui| {
        controls::instrument::key_strip(
            ui,
            controls::instrument::KeyStrip {
                rect: strip_rect,
                vertical: false,
                name: "Robotize mode",
                accent: ROBOT_ACCENT,
            },
            &mut state.robotize.mode,
            &[
                (
                    RobotizeMode::RingMod,
                    "Ring",
                    "16x oversampled carrier ring modulation.",
                ),
                (
                    RobotizeMode::FreqShift,
                    "Shift",
                    "Single-sideband upward frequency shift.",
                ),
            ],
        );
    });
    draw_dial_row(
        ui,
        rect,
        state.robotize.enabled,
        vec![
            (
                DialSpec {
                    name: "Color",
                    caption: "Color",
                    low: 1.0,
                    high: 20000.0,
                    reset: Robotize::default().frequency_hz,
                    step: 1.0,
                    decimals: 0,
                    logarithmic: true,
                    suffix: " Hz",
                    well_w: 72.0,
                },
                &mut state.robotize.frequency_hz,
            ),
            (
                DialSpec {
                    name: "Robotize volume",
                    caption: "Robotize volume",
                    low: 0.0,
                    high: 1.0,
                    reset: Robotize::default().wet,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 58.0,
                },
                &mut state.robotize.wet,
            ),
        ],
    );
}

fn draw_reverb_bay(ui: &mut egui::Ui, rect: Rect, state: &mut EffectsState) {
    let strip_rect = switchable_bay_header(
        ui,
        rect,
        "Reverb",
        REVERB_ACCENT,
        &mut state.reverb.enabled,
        "Turn stereo Reverb on or off.",
    );
    ui.add_enabled_ui(state.reverb.enabled, |ui| {
        controls::instrument::key_strip(
            ui,
            controls::instrument::KeyStrip {
                rect: strip_rect,
                vertical: false,
                name: "Reverb space",
                accent: REVERB_ACCENT,
            },
            &mut state.reverb.space,
            &[
                (
                    ReverbSpace::Room,
                    "Room",
                    "Compact ambient room reflections.",
                ),
                (ReverbSpace::Hall, "Hall", "Wide reverberant hall space."),
                (ReverbSpace::Plate, "Plate", "Dense bright studio plate."),
            ],
        );
    });
    draw_dial_row(
        ui,
        rect,
        state.reverb.enabled,
        vec![
            (
                DialSpec {
                    name: "Reverb decay",
                    caption: "Decay",
                    low: 0.2,
                    high: 8.0,
                    reset: Reverb::default().decay_s,
                    step: 0.05,
                    decimals: 2,
                    logarithmic: false,
                    suffix: " s",
                    well_w: 58.0,
                },
                &mut state.reverb.decay_s,
            ),
            (
                DialSpec {
                    name: "Reverb damping",
                    caption: "Damp",
                    low: 0.0,
                    high: 1.0,
                    reset: Reverb::default().damping,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 54.0,
                },
                &mut state.reverb.damping,
            ),
            (
                DialSpec {
                    name: "Reverb volume",
                    caption: "Wet",
                    low: 0.0,
                    high: 1.0,
                    reset: Reverb::default().wet,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 54.0,
                },
                &mut state.reverb.wet,
            ),
        ],
    );
}

fn draw_distortion_bay(ui: &mut egui::Ui, rect: Rect, state: &mut EffectsState) {
    let active = state.dirty > 0.0 || state.noise > 0.0 || (state.volume - 1.0).abs() > 1e-4;
    controls::instrument::bay_with_state(
        ui,
        rect,
        "Distortion / output",
        DISTORT_ACCENT,
        false,
        active,
    );
    let strip_rect = Rect::from_min_size(
        rect.min + Vec2::new(10.0, 24.0),
        Vec2::new((rect.width() - 20.0).max(60.0), 20.0),
    );
    controls::instrument::key_strip(
        ui,
        controls::instrument::KeyStrip {
            rect: strip_rect,
            vertical: false,
            name: "Distortion shape",
            accent: DISTORT_ACCENT,
        },
        &mut state.dist_shape,
        &[
            (
                DistortionShape::Warm,
                "Warm",
                "Tube-style warm asymmetric saturation.",
            ),
            (
                DistortionShape::Acid,
                "Acid",
                "Aggressive asymmetric acid overdrive.",
            ),
            (
                DistortionShape::Fuzz,
                "Fuzz",
                "Harder arctan fuzz saturation.",
            ),
            (
                DistortionShape::Crush,
                "Crush",
                "Bit-depth and sample-rate crusher.",
            ),
        ],
    );
    draw_dial_row(
        ui,
        rect,
        true,
        vec![
            (
                DialSpec {
                    name: "Dirty distortion",
                    caption: "Dirty distortion",
                    low: 0.0,
                    high: 1.0,
                    reset: EffectsState::default().dirty,
                    step: 0.001,
                    decimals: 3,
                    logarithmic: false,
                    suffix: "",
                    well_w: 58.0,
                },
                &mut state.dirty,
            ),
            (
                DialSpec {
                    name: "Noise distortion",
                    caption: "Noise distortion",
                    low: 0.0,
                    high: 1.0,
                    reset: EffectsState::default().noise,
                    step: 0.001,
                    decimals: 3,
                    logarithmic: false,
                    suffix: "",
                    well_w: 58.0,
                },
                &mut state.noise,
            ),
            (
                DialSpec {
                    name: "Output volume",
                    caption: "Output volume",
                    low: 0.0,
                    high: 2.0,
                    reset: EffectsState::default().volume,
                    step: 0.01,
                    decimals: 2,
                    logarithmic: false,
                    suffix: "",
                    well_w: 58.0,
                },
                &mut state.volume,
            ),
        ],
    );
}
