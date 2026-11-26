//! The board itself: the preset selector and bypass switch above it, the
//! scrolling row of pedals between the input and output units, the strip of
//! chips below it, and the status line that names the chain.
use super::*;

/// Lamp states that outlast a single reading: when each input peak lamp
/// last lit, and whether the OVER lamp has latched since it was cleared.
#[derive(Clone, Copy)]
pub(super) struct Lamps {
    pub(super) peaks: [f64; 2],
    pub(super) run: u32,
    pub(super) overs: u32,
    pub(super) over: bool,
}
impl Default for Lamps {
    fn default() -> Self {
        Self {
            peaks: [f64::NEG_INFINITY; 2],
            run: 0,
            overs: 0,
            over: false,
        }
    }
}

impl Lamps {
    // Latches outlast the identity of the project controls.
    const ID: &str = "master-lamps";

    pub(super) fn load(ctx: &egui::Context) -> Self {
        ctx.data(|data| data.get_temp(egui::Id::new(Self::ID)))
            .unwrap_or_default()
    }

    pub(super) fn store(self, ctx: &egui::Context) {
        ctx.data_mut(|data| data.insert_temp(egui::Id::new(Self::ID), self));
    }
}

/// What the scrolling row reports: the part of it in view, as fractions
/// of its length, the scroll offset that centres each pedal, and any card
/// whose Edit trigger opened Mastering deep edit.
struct RowView {
    visible: [f32; 2],
    centres: Vec<f32>,
    open_deep_edit: Option<PedalKind>,
}

/// The preset selector of the module toolbar. It names the preset whose
/// pedals the board's equal, or Custom once a pedal has changed, and returns
/// the board of a chosen preset. The trim is the mix's, so neither changes it.
pub(crate) fn preset_selector(
    ui: &mut egui::Ui,
    chain: &MasterChain,
    label: &str,
) -> Option<MasterChain> {
    let untouched = MasterChain {
        bypass: chain.bypass,
        trim: chain.trim,
        ..MasterChain::default()
    };
    let current = presets::matching(chain).unwrap_or(if *chain == untouched {
        "Choose a preset"
    } else {
        "Custom"
    });
    let mut chosen = None;
    let ctx = ui.ctx().clone();
    let combo = egui::ComboBox::from_id_salt("master_preset")
        .selected_text(current)
        .width(150.0)
        .height(560.0);
    let response = view::tall_menu(&ctx, || {
        combo.show_ui(ui, |ui| {
            for (index, preset) in presets::PRESETS.iter().enumerate() {
                // A rule parts the three general presets from the genres.
                if index == 3 {
                    ui.separator();
                }
                if ui
                    .selectable_label(preset.name == current, preset.name)
                    .clicked()
                {
                    chosen = Some(preset.applied_to(chain));
                    ui.close();
                }
            }
        })
    })
    .response;
    form::choice_named(&response, label, current);
    response.hover_hint(
        "Load a factory board. A preset sets every pedal and the pedal order, and turns \
         the Booster pedals off. Trim stays as you set it for the mix. Undo restores \
         the board before it.",
    );
    chosen
}

/// The module toolbar's Bypass all switch, which the Mixer's Master bypass
/// mirrors. True when it was clicked.
pub(crate) fn bypass_switch(ui: &mut egui::Ui, bypass: &mut bool) -> bool {
    let (rect, _) = ui.allocate_exact_size(Vec2::new(112.0, 28.0), Sense::hover());
    let response = controls::toggle_at(ui, rect, "Bypass all pedals", "Bypass all", bypass);
    let clicked = response.clicked();
    name_control(response, "Bypass all pedals").hover_hint(format!(
        "Bypass all: {}. Skip the whole board, trim included, for playback and export, so you \
         can master the exported mix elsewhere. The pedals keep their settings and switches; \
         presets leave this switch alone. The Mixer's Master bypass is the same switch.",
        if *bypass { "on" } else { "off" }
    ));
    clicked
}

/// Show the board in the space left in `ui`. `reveal` brings a pedal into
/// view, as its chip does.
pub(crate) fn show(
    ui: &mut egui::Ui,
    chain: &MasterChain,
    metering: &Metering,
    reveal: Option<PedalKind>,
) -> Action {
    let mut draft = *chain;
    if metering.playing {
        ui.ctx().request_repaint_after(metering.repaint_delay());
    }
    let area = ui.available_rect_before_wrap();
    ui.advance_cursor_after_rect(area);
    let mut board = ui.new_child(
        egui::UiBuilder::new()
            .id_salt("mastering-board")
            .max_rect(area),
    );
    dialog::hardware_style(&mut board, controls::Palette::Indigo);
    board.set_clip_rect(area.intersect(ui.clip_rect()));
    controls::panel(&board, area);
    named(
        &board.interact(area, board.id().with("surface"), Sense::hover()),
        egui::WidgetType::Other,
        "Mastering board",
    );
    let mut content = board.new_child(
        egui::UiBuilder::new()
            .id_salt("mastering-content")
            .max_rect(area.shrink(BOARD_INSET)),
    );
    // A bypassed board stays in reach but steps back: the mix skips it.
    if chain.bypass {
        content.multiply_opacity(0.45);
    }
    // A short section scrolls the board. Every control but the strip's
    // chips lives in the units and the row at its top, so bringing one of
    // them into view returns the board to the top, where all of it shows.
    let raise = board
        .ctx()
        .data_mut(|data| data.remove_temp::<bool>(raise_id()))
        .unwrap_or(false);
    let mut scroll = egui::ScrollArea::both()
        .id_salt("mastering-scroll")
        .auto_shrink([false, false]);
    if raise {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    let open_deep_edit = scroll
        .show(&mut content, |ui| {
            board_content(ui, chain, &mut draft, metering, reveal)
        })
        .inner;
    super::super::view::name_scroll_area(&content, "mastering-scroll", "Mastering");
    if draft != *chain {
        Action::Change(draft)
    } else if let Some(kind) = open_deep_edit {
        Action::OpenDeepEdit(kind)
    } else {
        Action::None
    }
}

fn board_content(
    ui: &mut egui::Ui,
    chain: &MasterChain,
    draft: &mut MasterChain,
    metering: &Metering,
    reveal: Option<PedalKind>,
) -> Option<PedalKind> {
    let minimum = 2.0 * MARGIN + INPUT_WIDTH + OUTPUT_WIDTH + 160.0;
    let size = Vec2::new(ui.available_width().max(minimum), BOARD_HEIGHT);
    let (bounds, _) = ui.allocate_exact_size(size, Sense::hover());
    let mut panel = ui.new_child(egui::UiBuilder::new().max_rect(bounds));
    let top = bounds.top();
    let input = Rect::from_min_size(
        Pos2::new(bounds.left() + MARGIN, top + PEDAL_TOP),
        Vec2::new(INPUT_WIDTH, PEDAL_HEIGHT),
    );
    let output = Rect::from_min_size(
        Pos2::new(bounds.right() - MARGIN - OUTPUT_WIDTH, top + PEDAL_TOP),
        Vec2::new(OUTPUT_WIDTH, PEDAL_HEIGHT),
    );
    let mut lamps = Lamps::load(panel.ctx());
    input_unit(&mut panel, input, draft, metering, &mut lamps);
    output_unit(&mut panel, output, metering, &mut lamps);
    lamps.store(panel.ctx());

    let viewport = Rect::from_min_max(
        Pos2::new(input.right(), top),
        Pos2::new(output.left(), top + ROW_HEIGHT),
    );
    let jump_id = panel.id().with("pedal-jump");
    let row = pedal_row(&mut panel, viewport, jump_id, chain, draft, metering);
    let strip_rect = Rect::from_min_size(
        Pos2::new(bounds.left() + MARGIN, top + ROW_HEIGHT),
        Vec2::new(bounds.width() - 2.0 * MARGIN, STRIP_HEIGHT),
    );
    let chosen = strip(&mut panel, strip_rect, draft, row.visible);
    let focus = chosen.map(|click| click.kind).or(reveal);
    if let Some(index) = focus.and_then(|kind| chain.order.iter().position(|k| *k == kind))
        && let Some(offset) = row.centres.get(index)
    {
        panel.ctx().data_mut(|data| {
            data.insert_temp(jump_id, *offset);
            data.insert_temp(raise_id(), true);
        });
        panel.ctx().request_repaint();
    }
    chosen
        .filter(|click| click.open)
        .map(|click| click.kind)
        .or(row.open_deep_edit)
}

/// The scrolling row of pedals between the two units.
fn pedal_row(
    panel: &mut egui::Ui,
    viewport: Rect,
    jump_id: egui::Id,
    chain: &MasterChain,
    draft: &mut MasterChain,
    metering: &Metering,
) -> RowView {
    let (lefts, natural) = row_layout(&chain.order);
    let row_width = natural.max(viewport.width());
    let jump = panel
        .ctx()
        .data_mut(|data| data.remove_temp::<f32>(jump_id));
    let mut row = panel.new_child(
        egui::UiBuilder::new()
            .id_salt("pedal-row")
            .max_rect(viewport),
    );
    // Scrolls to a control land at once, as the controls ask: a move
    // button that jumps a wide pedal stays in view on the next frame.
    let mut area = egui::ScrollArea::horizontal()
        .id_salt("pedal-row-scroll")
        .auto_shrink([false, false])
        .animated(false);
    if let Some(offset) = jump {
        area = area.horizontal_scroll_offset(offset);
    }
    let shown = area.show(&mut row, |ui| {
        let (content, _) = ui.allocate_exact_size(Vec2::new(row_width, ROW_HEIGHT), Sense::hover());
        let pedals = positioned_pedals(content, &chain.order, &lefts);
        draw_patch_cables(ui, content, &pedals);
        let (open_deep_edit, drag) = draw_pedals(ui, draft, &pedals, metering);
        if let Some(drag) = drag {
            apply_pedal_drag(ui, viewport, &pedals, drag, draft);
        }
        open_deep_edit
    });
    super::super::view::name_scroll_area(&row, "pedal-row-scroll", "Pedal row");
    let width = shown.inner_rect.width();
    let offset = shown.state.offset.x;
    let most = (row_width - width).max(0.0);
    row_shadows(panel, shown.inner_rect, offset, most);
    RowView {
        visible: [offset / row_width, ((offset + width) / row_width).min(1.0)],
        centres: chain
            .order
            .iter()
            .zip(&lefts)
            .map(|(kind, left)| (left + pedal_width(*kind) / 2.0 - width / 2.0).clamp(0.0, most))
            .collect(),
        open_deep_edit: shown.inner,
    }
}

fn positioned_pedals(content: Rect, order: &[PedalKind], lefts: &[f32]) -> Vec<(PedalKind, Rect)> {
    order
        .iter()
        .zip(lefts)
        .map(|(&kind, left)| {
            let min = Pos2::new(content.left() + left, content.top() + PEDAL_TOP);
            (
                kind,
                Rect::from_min_size(min, Vec2::new(pedal_width(kind), PEDAL_HEIGHT)),
            )
        })
        .collect()
}

fn draw_patch_cables(ui: &egui::Ui, content: Rect, pedals: &[(PedalKind, Rect)]) {
    let y = content.top() + SIGNAL_Y;
    let mut from = content.left();
    for (index, (_, rect)) in pedals.iter().enumerate() {
        cable(ui, from, rect.left(), y, [index != 0, true]);
        from = rect.right();
    }
    cable(ui, from, content.right(), y, [true, false]);
}

#[derive(Clone, Copy)]
struct PedalDrag {
    from: usize,
    released: bool,
    pointer: Pos2,
}

fn draw_pedals(
    ui: &mut egui::Ui,
    draft: &mut MasterChain,
    pedals: &[(PedalKind, Rect)],
    metering: &Metering,
) -> (Option<PedalKind>, Option<PedalDrag>) {
    let mut drag = None;
    let mut open = None;
    for (index, &(kind, rect)) in pedals.iter().enumerate() {
        let outcome = pedal(ui, draft, kind, rect, index, metering);
        if let Some(to) = outcome.moved_to {
            draft.move_pedal(index, to);
        }
        if outcome.open_deep_edit {
            open = Some(kind);
        }
        drag = pedal_grip(ui, rect, kind, index).or(drag);
    }
    (open, drag)
}

fn pedal_grip(ui: &egui::Ui, rect: Rect, kind: PedalKind, index: usize) -> Option<PedalDrag> {
    // The grip takes no Tab stop. The arrows move a pedal from the keyboard.
    let handle = Rect::from_min_size(rect.min, Vec2::new(rect.width() - 68.0, HEADER));
    let id = ui.id().with(("pedal-grip", kind));
    if !kind.movable() {
        let header = ui.interact(handle, id, Sense::hover());
        name_region(header, &format!("{} always runs last", name(kind)))
            .hover_hint("The limiter always runs last, just before the output.");
        return None;
    }
    let grip = name_region(
        ui.interact(handle, id, Sense::DRAG),
        &format!("Drag {}", name(kind)),
    )
    .on_hover_cursor(egui::CursorIcon::Grab)
    .hover_hint("Drag along the board to change where this pedal sits in the signal path.");
    if !controls::primary_drag_or_stop(ui, &grip) {
        return None;
    }
    let released = ui.input(|input| input.pointer.primary_released());
    if !released {
        ui.ctx().set_cursor_icon(egui::CursorIcon::Grabbing);
    }
    Some(PedalDrag {
        from: index,
        released,
        pointer: controls::drag_position(ui, &grip)?,
    })
}

fn apply_pedal_drag(
    ui: &egui::Ui,
    viewport: Rect,
    pedals: &[(PedalKind, Rect)],
    drag: PedalDrag,
    draft: &mut MasterChain,
) {
    if !drag.released {
        scroll_dragged_pedal(ui, viewport, drag.pointer.x);
    }
    let others: Vec<Rect> = pedals
        .iter()
        .enumerate()
        .filter(|(index, _)| *index != drag.from)
        .map(|(_, (_, rect))| *rect)
        .collect();
    // The limiter stays last.
    let to = others
        .iter()
        .take_while(|rect| rect.center().x < drag.pointer.x)
        .count()
        .min(others.len() - 1);
    let dragged = pedals[drag.from].1;
    let marker_x = if to == 0 {
        others[0].left() - CABLE / 2.0
    } else {
        others[to - 1].right() + CABLE / 2.0
    };
    let accent = ui.visuals().selection.bg_fill;
    ui.painter().rect_stroke(
        dragged,
        6.0,
        Stroke::new(2.0_f32, accent),
        egui::StrokeKind::Outside,
    );
    ui.painter().line_segment(
        [
            Pos2::new(marker_x, dragged.top()),
            Pos2::new(marker_x, dragged.bottom()),
        ],
        Stroke::new(3.0_f32, accent),
    );
    if drag.released {
        draft.move_pedal(drag.from, to);
    }
}

fn scroll_dragged_pedal(ui: &egui::Ui, viewport: Rect, pointer_x: f32) {
    let edge = 48.0;
    let left = (viewport.left() + edge - pointer_x).clamp(0.0, edge);
    let right = (pointer_x - (viewport.right() - edge)).clamp(0.0, edge);
    let push = left - right;
    if push != 0.0 {
        ui.scroll_with_delta_animation(
            Vec2::new(push * 0.4, 0.0),
            egui::style::ScrollAnimation::none(),
        );
        ui.ctx().request_repaint();
    }
}

fn row_shadows(panel: &egui::Ui, view: Rect, offset: f32, most: f32) {
    for (side, edge, more) in [
        (-1.0, view.left(), offset > 0.5),
        (1.0, view.right(), offset < most - 0.5),
    ] {
        if !more {
            continue;
        }
        for step in 0..8 {
            let inner = edge - side * step as f32 * 2.0;
            let strip = Rect::from_x_y_ranges(
                inner.min(inner - side * 2.0)..=inner.max(inner - side * 2.0),
                view.top() + PEDAL_TOP..=view.top() + PEDAL_TOP + PEDAL_HEIGHT,
            );
            panel
                .painter()
                .rect_filled(strip, 0.0, Color32::from_black_alpha(70 - 8 * step as u8));
        }
    }
}

#[derive(Clone, Copy)]
struct StripClick {
    kind: PedalKind,
    open: bool,
}

/// The strip under the board: every pedal in signal order between the mixer
/// and the output, lit when on, with the part of the row in view outlined.
/// A click reveals a pedal. A double click also opens its Deep Edit controls.
fn strip(
    ui: &mut egui::Ui,
    rect: Rect,
    chain: &MasterChain,
    visible: [f32; 2],
) -> Option<StripClick> {
    controls::well(ui, rect);
    let chips_top = rect.top() + 4.0;
    let ends = 52.0;
    let gap = 5.0;
    let room = rect.width() - 2.0 * ends - 16.0;
    let width = ((room - gap * 12.0) / 13.0).clamp(34.0, 70.0);
    let total = 13.0 * width + 12.0 * gap;
    let left = rect.center().x - total / 2.0;
    for (text, x, align) in [
        ("Mixer", left - 8.0, Align2::RIGHT_CENTER),
        ("Output", left + total + 8.0, Align2::LEFT_CENTER),
    ] {
        ui.painter().text(
            Pos2::new(x, chips_top + 11.0),
            align,
            text,
            FontId::proportional(11.5),
            roles().legend.gamma_multiply(0.8),
        );
    }
    let (lefts, end) = row_layout(&chain.order);
    let mut chosen = None;
    let mut seen: Option<Rect> = None;
    for (index, kind) in chain.order.iter().enumerate() {
        let chip = Rect::from_min_size(
            Pos2::new(left + index as f32 * (width + gap), chips_top),
            Vec2::new(width, 22.0),
        );
        let on = chain.is_on(*kind);
        let label = format!("Show {}", name(*kind));
        let response = signal_chip(ui, chip, *kind, on, false, &label).hover_hint(format!(
            "{}: {}. Click to show the pedal. Double-click to open Mastering deep edit.",
            name(*kind),
            if on { "on" } else { "off" }
        ));
        if response.clicked() {
            chosen = Some(StripClick {
                kind: *kind,
                open: response.double_clicked(),
            });
        }
        // The chips whose pedals are in view share an outline.
        let middle = (lefts[index] + pedal_width(*kind) / 2.0) / end;
        if (visible[0]..=visible[1]).contains(&middle) {
            seen = Some(seen.map_or(chip, |seen| seen.union(chip)));
        }
    }
    if let Some(seen) = seen {
        ui.painter().rect_stroke(
            seen.expand(3.0),
            6.0,
            Stroke::new(1.5_f32, roles().legend),
            egui::StrokeKind::Outside,
        );
    }

    chosen
}

/// The board's state in a line, and the signal path for its hover text.
pub(crate) fn status(chain: &MasterChain) -> (String, String) {
    let status = if chain.bypass {
        "Bypassed: the mix plays and exports without the trim and pedals.".to_owned()
    } else if chain.is_active() {
        active_status(chain)
    } else {
        "Every pedal is off and the trim is at 0 dB: the mix plays unprocessed.".to_owned()
    };
    let path: Vec<String> = chain
        .order
        .iter()
        .map(|kind| {
            if chain.is_on(*kind) {
                name(*kind).to_owned()
            } else {
                format!("{} (off)", name(*kind))
            }
        })
        .collect();
    (
        status,
        if chain.bypass {
            format!(
                "Signal: mixer → output. The bypassed board holds {}.",
                path.join(" → ")
            )
        } else {
            format!("Signal: mixer → {} → output.", path.join(" → "))
        },
    )
}

fn active_status(chain: &MasterChain) -> String {
    let on = chain
        .order
        .iter()
        .filter(|kind| chain.is_on(**kind))
        .count();
    let pedals = if on == 1 {
        "1 pedal on".to_owned()
    } else {
        format!("{on} of 13 pedals on")
    };
    let trim = if chain.trim != 0.0 {
        format!(", trim {:+.1} dB", chain.trim)
    } else {
        String::new()
    };
    let latency = chain.latency_ms(MIX_SAMPLE_RATE);
    let delay = if latency > 0.0 {
        format!(". Export removes {latency:.1} ms of delay.")
    } else {
        ".".to_owned()
    };
    format!("{pedals}{trim}{delay}")
}
