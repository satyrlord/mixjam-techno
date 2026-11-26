//! Floating Mastering Deep Edit window with the signal-order ribbon, pedal
//! controls, saturation curves, and master meter readings.
use super::*;

use mixjam_techno::application::LimiterStyle;

pub(in super::super) const DEEP_EDIT_TITLE: &str = "Mastering deep edit";
/// The workspace inside the window's inset. The window takes its size.
const WORKSPACE: Vec2 = Vec2::new(744.0, 432.0);
/// The height of one compact dial group, from the knob's top to the label's
/// foot, with the knob centre 24 points and the label 77 points below its top.
const DEEP_DIAL_H: f32 = 86.0;
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

pub(in super::super) struct DeepEditOutcome {
    pub(in super::super) chain: Option<MasterChain>,
    pub(in super::super) help: bool,
    pub(in super::super) discrete: bool,
}

pub(in super::super) fn show_deep_edit(
    ctx: &egui::Context,
    open: &mut bool,
    active: &mut PedalKind,
    chain: &MasterChain,
    metering: &Metering,
    generation: u64,
) -> DeepEditOutcome {
    if metering.playing {
        ctx.request_repaint_after(metering.repaint_delay());
    }
    let mut draft = *chain;
    let mut help = false;
    let mut discrete = false;
    let mut bounds = ctx.viewport_rect().shrink(12.0);
    bounds.min.y = bounds.min.y.max(68.0);
    dialog::window(DEEP_EDIT_TITLE)
        .open(open)
        .help(&mut help)
        .palette(controls::Palette::Indigo)
        .default_width(WORKSPACE.x + 36.0)
        .default_height(WORKSPACE.y + 12.0)
        .fixed()
        .constrain_to(bounds)
        .show(ctx, |ui| {
            // The window takes the extent of the painted controls.
            let (full, _) = ui.allocate_exact_size(WORKSPACE, Sense::hover());
            controls::panel(ui, full);
            let inner = full.shrink(10.0);
            let mut child =
                ui.new_child(egui::UiBuilder::new().id_salt(generation).max_rect(inner));
            discrete = workspace(&mut child, inner, active, &mut draft, metering);
        });
    DeepEditOutcome {
        chain: (draft != *chain).then_some(draft),
        help,
        discrete,
    }
}

fn step_pedal_key(ui: &egui::Ui, active: &mut PedalKind, chain: &MasterChain) {
    let idx = chain.order.iter().position(|k| *k == *active).unwrap_or(0);
    let alt = egui::Modifiers::ALT;
    let delta = ui.input_mut(|input| {
        if input.consume_key(alt, egui::Key::ArrowLeft) {
            -1_isize
        } else if input.consume_key(alt, egui::Key::ArrowRight) {
            1_isize
        } else {
            0
        }
    });
    if delta != 0 {
        let next = (idx as isize + delta).rem_euclid(chain.order.len() as isize) as usize;
        *active = chain.order[next];
    }
}

fn workspace(
    ui: &mut egui::Ui,
    rect: Rect,
    active: &mut PedalKind,
    chain: &mut MasterChain,
    metering: &Metering,
) -> bool {
    step_pedal_key(ui, active, chain);
    let header = Rect::from_min_size(rect.min, Vec2::new(rect.width(), 30.0));
    let discrete = header_bar(ui, *active, chain, header);
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
        let two_rows = *active == PedalKind::BusCompressor || *active == PedalKind::BoosterEq;
        let scope_h = if two_rows {
            (bay.height() - 224.0).clamp(96.0, 124.0)
        } else {
            (bay.height() - 210.0).min(152.0)
        };
        let scope = Rect::from_min_size(bay.min, Vec2::new(left_w, scope_h));
        scope_panel(ui, scope, *active, chain, metering);
        bay.min.y += scope_h + 8.0;
    }
    let telemetry = Rect::from_min_max(Pos2::new(bay.right() + 10.0, top), rect.max);
    parameter_bays(ui, bay, *active, chain);
    telemetry_column(ui, telemetry, *active, chain, metering);
    discrete
}

fn master_undo_cue_id() -> egui::Id {
    egui::Id::new("master-deep-undo-cue")
}

fn header_bar(ui: &mut egui::Ui, active: PedalKind, chain: &mut MasterChain, rect: Rect) -> bool {
    let mut discrete = false;
    let mut row = ui.new_child(egui::UiBuilder::new().max_rect(rect));
    row.horizontal(|ui| {
        if let Some(chosen) = preset_selector(ui, chain, "Deep edit preset") {
            *chain = chosen;
            controls::set_timed_cue(ui.ctx(), master_undo_cue_id(), "Ctrl+Z to undo");
            discrete = true;
        }
        ui.add_space(6.0);
        let (bypass_rect, _) = ui.allocate_exact_size(Vec2::new(94.0, 20.0), Sense::hover());
        ui.scope(|ui| {
            ui.visuals_mut().selection.bg_fill = roles().lamp_warn;
            controls::instrument::bat_toggle(
                ui,
                bypass_rect,
                "Deep edit bypass all",
                "Bypass all",
                &mut chain.bypass,
                "Skip the whole mastering board, trim included, for playback and export.",
                false,
            );
        });
        if let Some(text) = controls::timed_cue_text(ui.ctx(), master_undo_cue_id()) {
            ui.add_space(6.0);
            ui.label(
                egui::RichText::new(text)
                    .size(10.5)
                    .color(roles().legend.gamma_multiply(0.8)),
            );
        }
    });
    discrete | header_reorder_and_power(ui, active, chain, rect)
}

fn move_follow_id() -> egui::Id {
    egui::Id::new("deep-edit-move-follow")
}

fn reset_pedal_to_default(chain: &mut MasterChain, kind: PedalKind) {
    let keep_on = chain.is_on(kind);
    let defaults = MasterChain::default();
    for knob in kind.knobs() {
        knob.set(chain, knob.get(&defaults));
    }
    if kind == PedalKind::Tape {
        chain.tape.speed = defaults.tape.speed;
    }
    if kind == PedalKind::Limiter {
        chain.limiter.style = defaults.limiter.style;
    }
    chain.set_on(kind, keep_on);
}

fn pedal_matches_default(chain: &MasterChain, kind: PedalKind) -> bool {
    let mut reset = *chain;
    reset_pedal_to_default(&mut reset, kind);
    *chain == reset
}

fn follow_move_focus(ui: &egui::Ui, active: PedalKind, left: bool, resp: &egui::Response) {
    let follow = ui.data(|data| data.get_temp::<(PedalKind, bool, bool)>(move_follow_id()));
    if let Some((target, target_left, focus)) = follow
        && (target, target_left) == (active, left)
    {
        ui.ctx()
            .data_mut(|data| data.remove::<(PedalKind, bool, bool)>(move_follow_id()));
        if focus {
            resp.request_focus();
        }
    }
}

fn move_button_slot(
    ui: &mut egui::Ui,
    active: PedalKind,
    chain: &mut MasterChain,
    rect: Rect,
    index: usize,
    left: bool,
    enabled: bool,
) {
    let move_hint = if active.movable() {
        "Reorder the selected pedal in the mastering chain."
    } else {
        "The limiter always runs last in the mastering chain."
    };
    let (btn_label, arrow_dir, step) = if left {
        ("Deep edit move left", -1.0, -1_isize)
    } else {
        ("Deep edit move right", 1.0, 1_isize)
    };
    let x = rect.right() - if left { 174.0 } else { 144.0 };
    let btn_rect = Rect::from_min_size(Pos2::new(x, rect.center().y - 12.0), Vec2::new(26.0, 24.0));
    let resp = controls::button_at(
        ui,
        btn_rect,
        btn_label,
        "",
        controls::ButtonRole::Secondary,
        enabled,
    );
    arrow(ui, btn_rect.center(), arrow_dir, enabled);
    follow_move_focus(ui, active, left, &resp);
    if name_control(resp.clone(), btn_label)
        .hover_hint(move_hint)
        .disabled_hint(move_hint)
        .clicked()
    {
        let target = (index as isize + step) as usize;
        let keep_left = if left {
            target != 0
        } else {
            !chain.order.get(target + 1).is_some_and(|n| n.movable())
        };
        let focused = resp.has_focus();
        ui.data_mut(|data| data.insert_temp(move_follow_id(), (active, keep_left, focused)));
        chain.move_pedal(index, target);
    }
}

fn header_move_buttons(ui: &mut egui::Ui, active: PedalKind, chain: &mut MasterChain, rect: Rect) {
    let index = chain.order.iter().position(|k| *k == active).unwrap_or(0);
    let can_left = active.movable() && index > 0;
    let can_right = active.movable()
        && chain
            .order
            .get(index + 1)
            .is_some_and(|next| next.movable());
    move_button_slot(ui, active, chain, rect, index, true, can_left);
    move_button_slot(ui, active, chain, rect, index, false, can_right);
}

fn header_reorder_and_power(
    ui: &mut egui::Ui,
    active: PedalKind,
    chain: &mut MasterChain,
    rect: Rect,
) -> bool {
    let cy = rect.center().y;
    let label = name(active);
    let label_w = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(11.5), roles().legend)
        .size()
        .x
        .min(116.0);
    let div_x = rect.right() - 190.0 - label_w - 8.0;
    ui.painter().line_segment(
        [
            Pos2::new(div_x, rect.top() + 5.0),
            Pos2::new(div_x, rect.bottom() - 5.0),
        ],
        Stroke::new(1.0_f32, roles().legend.gamma_multiply(0.24)),
    );
    ui.painter().text(
        Pos2::new(rect.right() - 182.0, cy),
        Align2::RIGHT_CENTER,
        label,
        FontId::proportional(11.5),
        roles().legend.gamma_multiply(0.92),
    );
    header_move_buttons(ui, active, chain, rect);
    let mut on = chain.is_on(active);
    let sw_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 112.0, cy - 10.0),
        Vec2::new(56.0, 20.0),
    );
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
    let can_reset = !pedal_matches_default(chain, active);
    let reset_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 50.0, cy - 12.0),
        Vec2::new(50.0, 24.0),
    );
    let reset_resp = name_control(
        controls::button_at(
            ui,
            reset_rect,
            "Deep edit reset pedal",
            "Reset",
            controls::ButtonRole::Secondary,
            can_reset,
        ),
        "Deep edit reset pedal",
    );
    if reset_resp
        .hover_hint("Restore this pedal's settings to default. Undo restores it.")
        .disabled_hint("This pedal is already at its default settings.")
        .clicked()
    {
        reset_pedal_to_default(chain, active);
        controls::set_timed_cue(ui.ctx(), master_undo_cue_id(), "Ctrl+Z to undo");
        true
    } else {
        false
    }
}

fn chip_arrow_step(response: &egui::Response, index: usize, count: usize) -> Option<usize> {
    if !response.has_focus() || count == 0 {
        return None;
    }
    response.ctx.input_mut(|input| {
        if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowLeft) {
            Some((index + count - 1) % count)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowRight) {
            Some((index + 1) % count)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::Home) {
            Some(0)
        } else if input.consume_key(egui::Modifiers::NONE, egui::Key::End) {
            Some(count - 1)
        } else {
            None
        }
    })
}

fn chain_ribbon(ui: &mut egui::Ui, rect: Rect, active: &mut PedalKind, chain: &MasterChain) {
    controls::well(ui, rect);
    let count = chain.order.len();
    let gap = 4.0;
    let pad = 6.0;
    let chip_w = ((rect.width() - 2.0 * pad - gap * (count as f32 - 1.0)) / count as f32).max(32.0);
    let mut focus_target: Option<usize> = None;
    let mut chip_ids = Vec::with_capacity(count);
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
        chip_ids.push(resp.id);
        let state = format!(
            "Slot {} of {}, {}",
            i + 1,
            count,
            if on { "on" } else { "off" }
        );
        resp.ctx.accesskit_node_builder(resp.id, |node| {
            node.set_selected(selected);
            node.set_value(state);
        });
        if let Some(target_idx) = chip_arrow_step(&resp, i, count) {
            *active = chain.order[target_idx];
            focus_target = Some(target_idx);
        }
        if resp
            .hover_hint(format!(
                "{} ({}, slot {} of {}). Inspect and edit in Mastering deep edit.",
                name(*kind),
                if on { "on" } else { "off" },
                i + 1,
                count
            ))
            .clicked()
        {
            *active = *kind;
        }
    }
    if let Some(target) = focus_target {
        // The ribbon took the arrow, so egui must not move focus again.
        ui.ctx().memory_mut(|m| {
            m.request_focus(chip_ids[target]);
            m.move_focus(egui::FocusDirection::None);
        });
    }
}

fn has_deep_scope(kind: PedalKind) -> bool {
    matches!(
        kind,
        PedalKind::BoosterEq
            | PedalKind::SoftClip
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

fn deep_scope_state(
    kind: PedalKind,
    chain: &MasterChain,
    accent: Color32,
) -> controls::instrument::ScopeState<'static> {
    let active = chain.is_on(kind) && !chain.bypass;
    let status = if chain.bypass {
        Some("CHAIN BYPASSED")
    } else if !chain.is_on(kind) {
        Some("OFF")
    } else {
        None
    };
    controls::instrument::ScopeState {
        active,
        status,
        accent,
    }
}

fn draw_gr_history_scope(
    ui: &egui::Ui,
    rect: Rect,
    kind: PedalKind,
    chain: &MasterChain,
    metering: &Metering,
) {
    let state = deep_scope_state(kind, chain, master_motif(kind).scope_accent());
    let current = if !state.active || !metering.playing {
        0.0
    } else if kind == PedalKind::BusCompressor {
        metering.reading.compressor_reduction
    } else {
        metering.reading.limiter_reduction
    };
    let history = update_gr_history(ui, kind, current, metering);
    controls::instrument::gr_history_scope_state(
        ui,
        rect,
        &format!("{} GR history", name(kind)),
        &history,
        12.0,
        current,
        state,
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

fn eq_scope(
    ui: &egui::Ui,
    rect: Rect,
    kind: PedalKind,
    chain: &MasterChain,
    state: controls::instrument::ScopeState<'_>,
) {
    let mut curve = [0.0_f32; 65];
    mixjam_techno::application::sample_master_eq_curve(chain, kind, &mut curve);
    let (title, range_db) = match kind {
        PedalKind::BoosterEq => ("Booster EQ response", 18.0),
        PedalKind::TrimEq => ("Trim EQ response", 6.0),
        _ => ("Lift EQ response", 6.0),
    };
    controls::instrument::eq_response_scope_state(ui, rect, title, &curve, range_db, state);
}

fn scope_panel(
    ui: &egui::Ui,
    rect: Rect,
    kind: PedalKind,
    chain: &MasterChain,
    metering: &Metering,
) {
    let accent = master_motif(kind).scope_accent();
    let state = deep_scope_state(kind, chain, accent);
    match kind {
        PedalKind::BoosterEq | PedalKind::TrimEq | PedalKind::LiftEq => {
            eq_scope(ui, rect, kind, chain, state);
        }
        PedalKind::SoftClip | PedalKind::Tube | PedalKind::Maximizer | PedalKind::Tape => {
            let mut points = [0.0_f32; 65];
            mixjam_techno::application::sample_saturation_curve(chain, kind, &mut points);
            controls::instrument::scope_frame_state(
                ui,
                rect,
                &format!("{} transfer curve", name(kind)),
                &points,
                state,
            );
        }
        PedalKind::Imaging => draw_imaging_scope(ui, rect, chain, metering, accent),
        PedalKind::BusCompressor | PedalKind::Limiter => {
            draw_gr_history_scope(ui, rect, kind, chain, metering);
        }
        PedalKind::Multiband => {
            let bands = if state.active && metering.playing {
                metering.reading.multiband_reduction
            } else {
                [0.0; 3]
            };
            controls::instrument::multiband_gr_scope_state(
                ui,
                rect,
                "Multiband 3-band GR",
                bands,
                12.0,
                state,
            );
        }
        _ => {}
    }
}

struct BayControls<'a> {
    kind: PedalKind,
    knobs: &'a [Knob],
    anchors: &'a [(f32, f32)],
    cell_w: f32,
    body: Rect,
    knob_area: Rect,
}

fn draw_bay_controls(ui: &mut egui::Ui, chain: &mut MasterChain, bay: BayControls<'_>) {
    let motif = master_motif(bay.kind);
    ui.scope(|ui| {
        ui.visuals_mut().dark_mode = !motif.light_face;
        ui.visuals_mut().selection.bg_fill = motif.scope_accent();
        theme::with_knob_style(motif.knob, || {
            for (knob, &(cx, top)) in bay.knobs.iter().zip(bay.anchors) {
                draw_deep_knob(ui, *knob, cx, top, bay.cell_w, chain, true);
            }
            if bay.kind == PedalKind::Tape
                && let Some(&(cx, top)) = bay.anchors.get(bay.knobs.len())
            {
                draw_deep_tape_speed(ui, cx, top, bay.cell_w, chain, true);
            }
            if bay.kind == PedalKind::Limiter {
                let style_row = Rect::from_min_max(
                    Pos2::new(bay.body.left(), bay.knob_area.bottom() + 4.0),
                    bay.body.max,
                );
                limiter_style_selector(ui, style_row, &mut chain.limiter.style, true);
            }
        });
    });
}

fn bay_grid_geometry(
    rect: Rect,
    kind: PedalKind,
    total: usize,
) -> (Rect, Rect, f32, Vec<(f32, f32)>, Vec<Rect>) {
    let cols = if kind == PedalKind::BusCompressor {
        4
    } else {
        total.clamp(1, 5)
    };
    let rows = total.div_ceil(cols).max(1);
    let body = if kind == PedalKind::Limiter {
        Rect::from_min_max(
            rect.min + Vec2::new(16.0, 22.0),
            rect.max - Vec2::new(16.0, 14.0),
        )
    } else {
        Rect::from_min_max(
            rect.min + Vec2::new(10.0, if rows > 1 { 26.0 } else { 22.0 }),
            rect.max - Vec2::splat(10.0),
        )
    };
    let knob_area = if kind == PedalKind::Limiter {
        Rect::from_min_max(body.min, body.max - Vec2::new(0.0, 36.0))
    } else {
        body
    };
    let cell_w = knob_area.width() / cols as f32;
    let cell_h = knob_area.height() / rows as f32;
    // Each dial group sits in the middle of its row, not under the title.
    let inset = ((cell_h - DEEP_DIAL_H) / 2.0).max(0.0);
    let anchors: Vec<(f32, f32)> = (0..total)
        .map(|slot| {
            let (col, row) = (slot % cols, slot / cols);
            (
                knob_area.left() + (col as f32 + 0.5) * cell_w,
                knob_area.top() + row as f32 * cell_h + inset,
            )
        })
        .collect();
    let cells = anchors
        .iter()
        .map(|&(cx, top)| deep_cell(Pos2::new(cx, top + 24.0), top + 77.0, cell_w))
        .collect();
    (body, knob_area, cell_w, anchors, cells)
}

/// The bay's title, with the reason it is silent after it. The status sits
/// beside the title, because the motif art fills the right of the header.
fn bay_title(chain: &MasterChain, kind: PedalKind) -> String {
    if chain.bypass {
        format!("{} · Chain bypassed", name(kind))
    } else if !chain.is_on(kind) {
        format!("{} · Pedal off", name(kind))
    } else {
        name(kind).to_owned()
    }
}

fn parameter_bays(ui: &mut egui::Ui, rect: Rect, kind: PedalKind, chain: &mut MasterChain) {
    let motif = master_motif(kind);
    let bay_active = chain.is_on(kind) && !chain.bypass;
    let knobs = kind.knobs();
    let total = knobs.len() + usize::from(kind == PedalKind::Tape);
    let (body, knob_area, cell_w, anchors, cells) = bay_grid_geometry(rect, kind, total);
    paint_deep_edit_bay(
        ui,
        rect,
        &bay_title(chain, kind),
        motif,
        bay_active,
        kind as u64 + 1,
        Some(&cells),
    );
    draw_bay_controls(
        ui,
        chain,
        BayControls {
            kind,
            knobs,
            anchors: &anchors,
            cell_w,
            body,
            knob_area,
        },
    );
}

fn limiter_style_selector(ui: &mut egui::Ui, rect: Rect, style: &mut LimiterStyle, enabled: bool) {
    let text_color = if ui.visuals().dark_mode {
        roles().legend
    } else {
        roles().ink
    };
    ui.painter().text(
        Pos2::new(rect.left() + 6.0, rect.center().y),
        Align2::LEFT_CENTER,
        "Style",
        FontId::proportional(11.0),
        text_color,
    );
    let strip = Rect::from_min_max(
        Pos2::new(rect.left() + 50.0, rect.center().y - 12.0),
        Pos2::new(rect.right() - 6.0, rect.center().y + 12.0),
    );
    ui.add_enabled_ui(enabled, |ui| {
        controls::instrument::key_strip(
            ui,
            controls::instrument::KeyStrip {
                rect: strip,
                vertical: false,
                name: "Deep edit Limiter style",
                accent: master_motif(PedalKind::Limiter).scope_accent(),
            },
            style,
            &[
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
            ],
        );
    });
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
    controls::instrument::bay(
        ui,
        rect,
        "Master telemetry",
        master_motif(kind).scope_accent(),
        false,
    );
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
    ui.painter().text(
        over.right_center() - Vec2::new(8.0, 0.0),
        Align2::RIGHT_CENTER,
        if lamps.over {
            "Click to clear"
        } else {
            "Clear"
        },
        FontId::proportional(9.5),
        roles().legend.gamma_multiply(0.65),
    );
    lamps.store(ui.ctx());
}

fn format_telemetry(label: &str, value: f32, unit: &str) -> String {
    if unit.is_empty() {
        if value.is_finite() {
            format!("{value:+.2}")
        } else {
            "-".to_owned()
        }
    } else if label == "Gain reduction" || label == "Band GR" {
        if value.is_finite() {
            controls::instrument::reduction_text(value)
        } else {
            format!("- {unit}")
        }
    } else {
        format!("{} {unit}", show_level(value))
    }
}

const TELEMETRY_HINTS: [(&str, &str); 8] = [
    (
        "Input VU",
        "Pre-board mix loudness where 0 VU equals -18 dBFS.",
    ),
    (
        "Gain reduction",
        "Current gain reduction applied by the active dynamics pedal.",
    ),
    (
        "Band GR",
        "Peak gain reduction across the Low, Mid, and High bands.",
    ),
    (
        "Correlation",
        "Stereo phase correlation (-1 out of phase to +1 mono).",
    ),
    (
        "Momentary",
        "Short-window output loudness over the last 400 ms.",
    ),
    (
        "Short-term",
        "Output loudness integrated over the last 3 s.",
    ),
    (
        "Integrated",
        "Cumulative playback loudness across the current playback run.",
    ),
    (
        "True peak",
        "Maximum inter-sample true peak measured since playback started.",
    ),
];

fn telemetry_hint(label: &str) -> &'static str {
    TELEMETRY_HINTS
        .iter()
        .find(|&&(k, _)| k == label)
        .map_or("Master meter reading.", |&(_, hint)| hint)
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
    let text = format_telemetry(label, value, unit);
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
    readout.hover_hint(telemetry_hint(label));
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
