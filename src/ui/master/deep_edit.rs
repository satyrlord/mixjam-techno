//! Floating Mastering Deep Edit window with the signal-order ribbon, pedal
//! controls, saturation curves, and master meter readings.
use super::*;

use mixjam_techno::application::LimiterStyle;

pub(in super::super) const DEEP_EDIT_TITLE: &str = "Mastering deep edit";
const WINDOW_WIDTH: f32 = 780.0;
// The shell adds 68 points outside this height and uses 12 inside for its inset.
const WINDOW_BODY_HEIGHT: f32 = 412.0;
const WORKSPACE_MIN: Vec2 = Vec2::new(650.0, 400.0);
const GR_HISTORY_LEN: usize = 96;
/// Seconds between two points of a gain-reduction history.
const GR_HISTORY_STEP: f64 = 0.06;

/// The measured gain reductions of one playback, oldest first.
#[derive(Clone)]
struct GrTrace {
    samples: [f32; GR_HISTORY_LEN],
    run: u32,
    /// When the newest point was taken, before the first point `None`.
    last_t: Option<f64>,
}

impl Default for GrTrace {
    fn default() -> Self {
        Self {
            samples: [0.0; GR_HISTORY_LEN],
            run: 0,
            last_t: None,
        }
    }
}

impl GrTrace {
    /// Add the current reduction once per step. A new playback starts empty.
    fn record(&mut self, run: u32, current_db: f32, now: f64) {
        if self.run != run {
            *self = Self {
                run,
                ..Self::default()
            };
        }
        if self.last_t.is_none_or(|last| now - last >= GR_HISTORY_STEP) {
            self.samples.copy_within(1.., 0);
            self.samples[GR_HISTORY_LEN - 1] = current_db;
            self.last_t = Some(now);
        }
    }
}

pub(in super::super) fn show_deep_edit(
    ctx: &egui::Context,
    open: &mut bool,
    active: &mut PedalKind,
    chain: &MasterChain,
    metering: &Metering,
    live_oversampling_4x: bool,
    generation: u64,
) -> Option<MasterChain> {
    if metering.playing {
        ctx.request_repaint_after(metering.repaint_delay());
    }
    let mut draft = *chain;
    let bounds = ctx.viewport_rect().shrink(12.0);
    dialog::window(DEEP_EDIT_TITLE)
        .open(open)
        .palette(controls::Palette::Indigo)
        .default_width(WINDOW_WIDTH)
        .default_height(WINDOW_BODY_HEIGHT)
        .constrain_to(bounds)
        .show(ctx, |ui| {
            // The dialog's scroll area needs the extent of the painted controls.
            let size = ui.available_size().max(WORKSPACE_MIN);
            let (full, _) = ui.allocate_exact_size(size, Sense::hover());
            controls::panel(ui, full);
            let inner = full.shrink(10.0);
            let mut child =
                ui.new_child(egui::UiBuilder::new().id_salt(generation).max_rect(inner));
            workspace(
                &mut child,
                inner,
                active,
                &mut draft,
                metering,
                live_oversampling_4x,
            );
        });
    (draft != *chain).then_some(draft)
}

fn workspace(
    ui: &mut egui::Ui,
    rect: Rect,
    active: &mut PedalKind,
    chain: &mut MasterChain,
    metering: &Metering,
    live_oversampling_4x: bool,
) {
    let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0));
    header_bar(ui, *active, chain, live_oversampling_4x, header);
    let ribbon = Rect::from_min_size(
        rect.min + Vec2::new(0.0, 34.0),
        Vec2::new(rect.width(), 28.0),
    );
    chain_ribbon(ui, ribbon, active, chain);

    let top = ribbon.bottom() + 8.0;
    let left_w = (rect.width() - 270.0).clamp(400.0, 476.0);
    let mut bay = Rect::from_min_max(
        Pos2::new(rect.left(), top),
        Pos2::new(rect.left() + left_w, rect.bottom()),
    );
    if has_deep_scope(*active) {
        let two_rows = *active == PedalKind::BusCompressor;
        let scope_h = if two_rows {
            (bay.height() - 208.0).clamp(96.0, 140.0)
        } else {
            (bay.height() - 194.0).min(152.0)
        };
        let scope = Rect::from_min_size(bay.min, Vec2::new(left_w, scope_h));
        scope_panel(ui, scope, *active, chain, metering);
        bay.min.y += scope_h + 8.0;
    }
    let telemetry = Rect::from_min_max(Pos2::new(bay.right() + 10.0, top), rect.max);
    parameter_bays(ui, bay, *active, chain);
    telemetry_column(ui, telemetry, *active, chain, metering);
}

fn header_bar(
    ui: &mut egui::Ui,
    active: PedalKind,
    chain: &mut MasterChain,
    live_oversampling_4x: bool,
    rect: Rect,
) {
    let mut row = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    row.horizontal(|ui| {
        if let Some(chosen) = preset_selector(ui, chain, "Deep edit preset") {
            *chain = chosen;
        }
        ui.add_space(6.0);
        let bypass_rect = Rect::from_min_size(ui.cursor().min, Vec2::new(98.0, 26.0));
        ui.allocate_rect(bypass_rect, Sense::hover());
        let bypass = controls::toggle_at(
            ui,
            bypass_rect,
            "Deep edit bypass all",
            "Bypass all",
            &mut chain.bypass,
        );
        name_control(bypass, "Deep edit bypass all");
        ui.add_space(6.0);
        os_pill(ui, live_oversampling_4x);
        ui.add_space(8.0);
        header_reorder_and_power(ui, active, chain);
    });
}

fn os_pill(ui: &mut egui::Ui, live_oversampling_4x: bool) {
    let label = if live_oversampling_4x {
        "Live OS: 4x"
    } else {
        "Live OS: Off"
    };
    let (pill_rect, _) = ui.allocate_exact_size(Vec2::new(82.0, 24.0), Sense::hover());
    controls::well(ui, pill_rect);
    ui.painter().text(
        pill_rect.center(),
        Align2::CENTER_CENTER,
        label,
        FontId::proportional(10.5),
        roles().legend,
    );
    let resp = ui.interact(pill_rect, ui.id().with("deep-edit-os"), Sense::hover());
    named(&resp, egui::WidgetType::Label, label);
    resp.hover_hint("Mastering saturation runs at 8x with a linear-phase filter during export.");
}

fn header_reorder_and_power(ui: &mut egui::Ui, active: PedalKind, chain: &mut MasterChain) {
    let index = chain.order.iter().position(|k| *k == active).unwrap_or(0);
    let can_left = active.movable() && index > 0;
    let can_right = active.movable()
        && chain
            .order
            .get(index + 1)
            .is_some_and(|next| next.movable());
    for (label, arrow_dir, step, enabled) in [
        ("Deep edit move left", -1.0, -1_isize, can_left),
        ("Deep edit move right", 1.0, 1_isize, can_right),
    ] {
        let (btn_rect, _) = ui.allocate_exact_size(Vec2::new(26.0, 24.0), Sense::hover());
        let resp = controls::button_at(
            ui,
            btn_rect,
            label,
            "",
            controls::ButtonRole::Secondary,
            enabled,
        );
        arrow(ui, btn_rect.center(), arrow_dir, enabled);
        if name_control(resp, label)
            .hover_hint("Reorder the selected pedal in the mastering chain.")
            .clicked()
        {
            let target = (index as isize + step) as usize;
            chain.move_pedal(index, target);
        }
    }
    ui.add_space(6.0);
    let mut on = chain.is_on(active);
    let (sw_rect, _) = ui.allocate_exact_size(Vec2::new(78.0, 22.0), Sense::hover());
    controls::instrument::bat_toggle(
        ui,
        sw_rect,
        "Deep edit pedal power",
        if on { "On" } else { "Off" },
        &mut on,
        "Switch the active mastering pedal in or out of the chain.",
        false,
    );
    chain.set_on(active, on);
}

fn chain_ribbon(ui: &mut egui::Ui, rect: Rect, active: &mut PedalKind, chain: &MasterChain) {
    controls::well(ui, rect);
    let count = chain.order.len() as f32;
    let gap = 4.0;
    let pad = 6.0;
    let chip_w = ((rect.width() - 2.0 * pad - gap * (count - 1.0)) / count).max(32.0);
    for (i, kind) in chain.order.iter().enumerate() {
        let chip = Rect::from_min_size(
            Pos2::new(
                rect.left() + pad + i as f32 * (chip_w + gap),
                rect.top() + 3.0,
            ),
            Vec2::new(chip_w, rect.height() - 6.0),
        );
        let on = chain.is_on(*kind);
        let selected = *kind == *active;
        let label = format!("Deep edit select {}", name(*kind));
        let resp = signal_chip(ui, chip, *kind, on, selected, &label);
        resp.ctx
            .accesskit_node_builder(resp.id, |node| node.set_selected(selected));
        if resp
            .hover_hint(format!(
                "Inspect and edit {} in Mastering deep edit.",
                name(*kind)
            ))
            .clicked()
        {
            *active = *kind;
        }
    }
}

fn has_deep_scope(kind: PedalKind) -> bool {
    matches!(
        kind,
        PedalKind::SoftClip
            | PedalKind::Tube
            | PedalKind::TrimEq
            | PedalKind::Maximizer
            | PedalKind::LiftEq
            | PedalKind::Tape
            | PedalKind::Imaging
            | PedalKind::BusCompressor
            | PedalKind::Multiband
            | PedalKind::Limiter
    )
}

fn update_gr_history(
    ui: &egui::Ui,
    kind: PedalKind,
    current_db: f32,
    metering: &Metering,
) -> [f32; GR_HISTORY_LEN] {
    let now = ui.ctx().input(|input| input.time);
    let id = ui.id().with(("deep-gr-history", kind));
    let mut trace: GrTrace = ui.ctx().data(|data| data.get_temp(id)).unwrap_or_default();
    if metering.playing {
        trace.record(metering.reading.run, current_db, now);
    } else {
        trace = GrTrace::default();
    }
    ui.ctx()
        .data_mut(|data| data.insert_temp(id, trace.clone()));
    trace.samples
}

fn draw_gr_history_scope(
    ui: &egui::Ui,
    rect: Rect,
    kind: PedalKind,
    chain: &MasterChain,
    metering: &Metering,
) {
    let current = if !chain.is_on(kind) || !metering.playing {
        0.0
    } else if kind == PedalKind::BusCompressor {
        metering.reading.compressor_reduction
    } else {
        metering.reading.limiter_reduction
    };
    let history = update_gr_history(ui, kind, current, metering);
    controls::instrument::gr_history_scope(
        ui,
        rect,
        &format!("{} GR history", name(kind)),
        &history,
        12.0,
        current,
        family_accent(kind),
    );
}

fn draw_imaging_scope(
    ui: &egui::Ui,
    rect: Rect,
    chain: &MasterChain,
    metering: &Metering,
    accent: Color32,
) {
    // The meters measure the correlation after the whole board, so it is
    // valid with Imaging off, as the vectorscope points and telemetry are.
    let corr = if metering.playing {
        metering.reading.stereo_correlation
    } else {
        0.0
    };
    controls::instrument::stereo_vectorscope(
        ui,
        rect,
        "Stereo imaging scope",
        &metering.vectorscope,
        corr,
        chain.imaging.mono_below,
        accent,
    );
}

fn scope_panel(
    ui: &egui::Ui,
    rect: Rect,
    kind: PedalKind,
    chain: &MasterChain,
    metering: &Metering,
) {
    let accent = family_accent(kind);
    match kind {
        PedalKind::SoftClip | PedalKind::Tube | PedalKind::Maximizer | PedalKind::Tape => {
            let mut points = [0.0_f32; 65];
            mixjam_techno::application::sample_saturation_curve(chain, kind, &mut points);
            controls::instrument::scope_frame(
                ui,
                rect,
                &format!("{} transfer curve", name(kind)),
                &points,
                accent,
            );
        }
        PedalKind::TrimEq | PedalKind::LiftEq => {
            let mut curve = [0.0_f32; 65];
            mixjam_techno::application::sample_master_eq_curve(chain, kind, &mut curve);
            controls::instrument::eq_response_scope(
                ui,
                rect,
                &format!("{} response", name(kind)),
                &curve,
                6.0,
                chain.is_on(kind),
                accent,
            );
        }
        PedalKind::Imaging => draw_imaging_scope(ui, rect, chain, metering, accent),
        PedalKind::BusCompressor | PedalKind::Limiter => {
            draw_gr_history_scope(ui, rect, kind, chain, metering);
        }
        PedalKind::Multiband => {
            let bands = if chain.is_on(kind) && metering.playing {
                metering.reading.multiband_reduction
            } else {
                [0.0; 3]
            };
            controls::instrument::multiband_gr_scope(
                ui,
                rect,
                "Multiband 3-band GR",
                bands,
                12.0,
                accent,
            );
        }
        _ => {}
    }
}

fn parameter_bays(ui: &mut egui::Ui, rect: Rect, kind: PedalKind, chain: &mut MasterChain) {
    let accent = family_accent(kind);
    let title_text = format!("{} controls", name(kind));
    controls::instrument::bay(ui, rect, &title_text, accent, false);
    let on = chain.is_on(kind);
    let knobs = kind.knobs();
    let total = knobs.len() + usize::from(kind == PedalKind::Tape);
    let cols = if kind == PedalKind::BusCompressor {
        4
    } else {
        total.clamp(1, 5)
    };
    let rows = total.div_ceil(cols).max(1);
    let body = Rect::from_min_max(rect.min + Vec2::new(8.0, 22.0), rect.max - Vec2::splat(6.0));
    let knob_area = if kind == PedalKind::Limiter {
        Rect::from_min_max(body.min, body.max - Vec2::new(0.0, 40.0))
    } else {
        body
    };
    let cell_w = knob_area.width() / cols as f32;
    let cell_h = knob_area.height() / rows as f32;

    ui.scope(|ui| {
        for (slot, knob) in knobs.iter().enumerate() {
            let (col, row) = (slot % cols, slot / cols);
            let cx = knob_area.left() + (col as f32 + 0.5) * cell_w;
            let top = knob_area.top() + row as f32 * cell_h;
            draw_deep_knob(ui, *knob, cx, top, cell_w, chain, on);
        }
        if kind == PedalKind::Tape {
            let cx = knob_area.left() + 1.5 * cell_w;
            draw_deep_tape_speed(ui, cx, knob_area.top(), cell_w, chain, on);
        }
        if kind == PedalKind::Limiter {
            let style_row =
                Rect::from_min_max(Pos2::new(body.left(), knob_area.bottom() + 4.0), body.max);
            limiter_style_selector(ui, style_row, &mut chain.limiter.style, on);
        }
    });
}

fn limiter_style_selector(ui: &mut egui::Ui, rect: Rect, style: &mut LimiterStyle, enabled: bool) {
    ui.painter().text(
        Pos2::new(rect.left() + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Style",
        FontId::proportional(11.0),
        roles().legend,
    );
    let choices = [
        (
            LimiterStyle::Transparent,
            "Transparent",
            "Clean dual-stage limiting without pre-clipping.",
        ),
        (
            LimiterStyle::Punchy,
            "Punchy",
            "Fast transient recovery with light soft shaving to keep drum snap.",
        ),
        (
            LimiterStyle::Modern,
            "Modern",
            "Tightly linked release and soft micro-shave for dense club loudness.",
        ),
    ];
    let start_x = rect.left() + 50.0;
    let gap = 8.0;
    let btn_w = ((rect.width() - 56.0 - gap * 2.0) / 3.0).max(76.0);
    for (idx, (candidate, label, hint)) in choices.into_iter().enumerate() {
        let btn = Rect::from_center_size(
            Pos2::new(
                start_x + (idx as f32 + 0.5) * btn_w + idx as f32 * gap,
                rect.center().y,
            ),
            Vec2::new(btn_w, 24.0),
        );
        let selected = *style == candidate;
        let access_name = format!("Deep edit Limiter style {label}");
        let role = if selected {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        };
        let resp = controls::button_at(ui, btn, &access_name, label, role, enabled);
        resp.ctx
            .accesskit_node_builder(resp.id, |node| node.set_selected(selected));
        if name_control(resp, &access_name).hover_hint(hint).clicked() {
            *style = candidate;
        }
    }
}

fn draw_deep_knob(
    ui: &mut egui::Ui,
    knob: Knob,
    cx: f32,
    top: f32,
    cell_w: f32,
    chain: &mut MasterChain,
    on: bool,
) {
    let (label, knob_name, hint, format) = face(knob);
    let deep_name = format!("Deep edit {knob_name}");
    let before = decimal(knob.get(chain));
    let mut value = before;
    compact_dial(
        ui,
        [
            Pos2::new(cx, top + 24.0),
            Pos2::new(cx, top + 58.0),
            Pos2::new(cx, top + 77.0),
        ],
        &mut value,
        Dial {
            name: &deep_name,
            label: &label,
            hint,
            range: knob.range(),
            format,
            choices: &[],
        },
        on,
        deep_well(cell_w),
    );
    if value != before {
        knob.set(chain, value as f32);
    }
}

/// The value well of each compact dial in a Deep Edit grid of `cell_w` columns.
fn deep_well(cell_w: f32) -> f32 {
    (cell_w - 8.0).clamp(44.0, COMPACT_WELL + 10.0)
}

fn draw_deep_tape_speed(
    ui: &mut egui::Ui,
    cx: f32,
    top: f32,
    cell_w: f32,
    chain: &mut MasterChain,
    on: bool,
) {
    tape_speed_dial(
        ui,
        [
            Pos2::new(cx, top + 24.0),
            Pos2::new(cx, top + 58.0),
            Pos2::new(cx, top + 77.0),
        ],
        chain,
        "Deep edit Tape speed",
        on,
        Some(deep_well(cell_w)),
    );
}

fn telemetry_column(
    ui: &mut egui::Ui,
    rect: Rect,
    kind: PedalKind,
    chain: &mut MasterChain,
    metering: &Metering,
) {
    controls::instrument::bay(ui, rect, "Master telemetry", family_accent(kind), false);
    trim_dial(
        ui,
        [56.0, 92.0, 114.0].map(|y| Pos2::new(rect.center().x, rect.top() + y)),
        chain,
        "Deep edit Trim",
    );
    let rows = telemetry_rows(kind, metering);
    for (index, (label, value, unit)) in rows.enumerate() {
        let y = rect.top() + 140.0 + index as f32 * 24.0;
        telemetry_readout(ui, rect, y, label, value, unit);
    }
    let over = Rect::from_min_size(
        Pos2::new(rect.left() + 14.0, rect.bottom() - 34.0),
        Vec2::new(rect.width() - 28.0, 24.0),
    );
    controls::well(ui, over);
    let mut lamps = Lamps::load(ui.ctx());
    over_control(
        ui,
        over,
        "Deep edit Clear OVER",
        &metering.reading,
        &mut lamps,
        roles().legend,
    );
    lamps.store(ui.ctx());
}

fn telemetry_readout(ui: &egui::Ui, column: Rect, y: f32, label: &str, value: f32, unit: &str) {
    let pos = Pos2::new(column.left() + 14.0, y);
    ui.painter().text(
        pos,
        Align2::LEFT_CENTER,
        label,
        FontId::proportional(11.5),
        roles().legend,
    );
    let well = Rect::from_center_size(Pos2::new(column.right() - 46.0, y), Vec2::new(72.0, 20.0));
    controls::well(ui, well);
    let text = if unit.is_empty() {
        if value.is_finite() {
            format!("{value:+.2}")
        } else {
            "—".to_owned()
        }
    } else {
        format!("{} {unit}", show_level(value))
    };
    ui.painter().text(
        well.center(),
        Align2::CENTER_CENTER,
        &text,
        FontId::proportional(11.5),
        if unit == "dBTP" && value > OVER_DBTP {
            roles().lamp_peak
        } else {
            roles().legend
        },
    );
    let readout = ui.interact(
        Rect::from_min_max(Pos2::new(pos.x, well.top()), well.max),
        ui.id().with(("deep-meter", label)),
        Sense::hover(),
    );
    named(
        &readout,
        egui::WidgetType::Label,
        &format!("{label}: {text}"),
    );
}

fn pedal_telemetry_row(
    kind: PedalKind,
    reading: &MeterReading,
) -> Option<(&'static str, f32, &'static str)> {
    match kind {
        PedalKind::Imaging => Some(("Correlation", reading.stereo_correlation, "")),
        PedalKind::BusCompressor => Some(("Gain reduction", reading.compressor_reduction, "dB")),
        PedalKind::Multiband => {
            let peak = reading
                .multiband_reduction
                .into_iter()
                .fold(0.0_f32, f32::max);
            Some(("Band GR", peak, "dB"))
        }
        PedalKind::Limiter => Some(("Gain reduction", reading.limiter_reduction, "dB")),
        _ => None,
    }
}

fn telemetry_rows(
    kind: PedalKind,
    metering: &Metering,
) -> impl Iterator<Item = (&'static str, f32, &'static str)> {
    let reading = &metering.reading;
    let live = |v| {
        if metering.playing && reading.run > 0 {
            v
        } else {
            f32::NEG_INFINITY
        }
    };
    [("Input VU", live(reading.input_vu), "VU")]
        .into_iter()
        .chain(pedal_telemetry_row(kind, reading).map(|(label, gr, unit)| (label, live(gr), unit)))
        .chain([
            ("Momentary", live(reading.momentary), "LUFS"),
            ("Short-term", live(reading.short_term), "LUFS"),
            ("Integrated", reading.integrated, "LUFS"),
            ("True peak", reading.true_peak_max, "dBTP"),
        ])
}

#[cfg(test)]
mod gr_trace_tests {
    use super::{GR_HISTORY_LEN, GR_HISTORY_STEP, GrTrace};

    #[test]
    fn the_gain_reduction_history_holds_only_measured_points() {
        let mut trace = GrTrace::default();
        trace.record(1, 3.0, 0.0);
        // Only the reading appears; the older points stay empty.
        assert_eq!(trace.samples[GR_HISTORY_LEN - 1], 3.0);
        assert!(
            trace.samples[..GR_HISTORY_LEN - 1]
                .iter()
                .all(|db| *db == 0.0)
        );
        // A redraw within one step adds no point.
        trace.record(1, 5.0, GR_HISTORY_STEP / 2.0);
        assert_eq!(trace.samples[GR_HISTORY_LEN - 2..], [0.0, 3.0]);
        trace.record(1, 1.5, GR_HISTORY_STEP);
        assert_eq!(trace.samples[GR_HISTORY_LEN - 3..], [0.0, 3.0, 1.5]);
        // The next playback starts empty.
        trace.record(2, 0.5, 2.0 * GR_HISTORY_STEP);
        assert_eq!(trace.samples[GR_HISTORY_LEN - 1], 0.5);
        assert!(
            trace.samples[..GR_HISTORY_LEN - 1]
                .iter()
                .all(|db| *db == 0.0)
        );
    }
}
