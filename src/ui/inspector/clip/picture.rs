//! The picture at the top of a clip page: the clip at the panel's width with
//! its fades and gain line, and the fade handles that drag.
use super::super::super::view::TICKS_PER_BEAT;
use super::*;
use mixjam_techno::application::Fades;

/// Where the picture is and what its handles measure against.
struct Picture {
    rect: Rect,
    /// The width of one tick on the picture.
    tick_width: f32,
    /// The tick where the clip's sound ends, counted from the clip's start.
    sound_end: u32,
    /// The x where the two handles share their hit areas.
    split: f32,
    /// Which handle is the left one: 0 is the fade in handle.
    left: usize,
}

impl MixJamApp {
    /// The clip at the panel's width: its category fill and waveform, its
    /// fades shaded, and its gain line. The fade handles drag, as on the
    /// clip in the score, and change `fades`.
    pub(super) fn clip_picture(
        &mut self,
        ui: &mut egui::Ui,
        clip: &Clip,
        sample: &Sample,
        bpm: f64,
        fades: &mut Fades,
    ) {
        let width = ui.available_width();
        let (rect, _) = ui.allocate_exact_size(Vec2::new(width, PICTURE), Sense::hover());
        let length = clip.length_ticks.max(1);
        let tick_width = rect.width() / length as f32;
        let bar_width = tick_width * TICKS_PER_BAR as f32;
        let color = group_color(&sample.group_name);
        let painter = ui.painter().clone();
        theme::surface(&painter, rect, view::clip_radius(rect), color);
        let waves = &mut self.clip_paint.inspector_waves;
        waves.retain(|id, _| *id == clip.id);
        if let Some(wave) = view::clip_wave(waves, &self.session, clip, sample, tick_width, bpm) {
            waveform::paint_clip_wave(&painter, rect, rect.top(), wave, color);
        }
        // A tick on the bottom edge at each bar line, when bars are wide
        // enough to count.
        if bar_width >= 8.0 {
            let ink = roles().on_light.gamma_multiply(0.5);
            for bar in 1..clip.length_ticks.div_ceil(TICKS_PER_BAR) {
                let x = rect.left() + bar as f32 * bar_width;
                painter.line_segment(
                    [
                        Pos2::new(x, rect.bottom() - 5.0),
                        Pos2::new(x, rect.bottom() - 1.0),
                    ],
                    Stroke::new(1.0_f32, ink),
                );
            }
        }
        view::draw_fades(&painter, rect, clip, Some(sample), bar_width, bpm);
        view::draw_gain_line(&painter, rect, clip.gain);
        view::draw_fade_handles(&painter, rect, clip, sample, bar_width, bpm);
        if clip.reverse {
            painter.text(
                rect.right_bottom() + Vec2::new(-6.0, -4.0),
                Align2::RIGHT_BOTTOM,
                "rev",
                FontId::proportional(10.0),
                roles().on_light,
            );
        }

        let handles = view::fade_handles(rect, clip, sample, bar_width, bpm);
        let picture = Picture {
            rect,
            tick_width,
            sound_end: view::sound_ticks(clip, sample, bpm),
            split: (handles[0].x + handles[1].x) / 2.0,
            left: usize::from(handles[1].x < handles[0].x),
        };
        for (index, (edge, center)) in ["in", "out"].into_iter().zip(handles).enumerate() {
            fade_handle(ui, &picture, clip, fades, (index, edge), center);
        }
    }
}

/// One fade handle of the picture: its hit area, its drag, its keys, and its
/// accessibility node. An edit changes `fades`. `(index, edge)` is the handle's
/// index (0 for the fade in) and its name.
fn fade_handle(
    ui: &mut egui::Ui,
    picture: &Picture,
    clip: &Clip,
    fades: &mut Fades,
    (index, edge): (usize, &str),
    center: Pos2,
) {
    let Picture {
        rect,
        tick_width,
        sound_end,
        split,
        left,
    } = *picture;
    let name = format!("{CLIP} fade {edge} handle");
    let mut hit = rect.intersect(Rect::from_center_size(
        center,
        Vec2::splat(view::FADE_HANDLE + 8.0),
    ));
    // Split overlapping targets at the midpoint. At the same x, the
    // left and right halves still select different handles.
    if index == left {
        hit.max.x = hit.max.x.min(split);
    } else {
        hit.min.x = hit.min.x.max(split);
    }
    let response = ui.interact(hit, ui.id().with(&name), Sense::click_and_drag());
    let dragging = controls::primary_drag_or_stop(ui, &response);
    if response.hovered() || dragging {
        ui.ctx().set_cursor_icon(egui::CursorIcon::ResizeHorizontal);
    }
    if response.clicked() || (dragging && response.drag_started()) {
        response.request_focus();
    }
    let fine = ui.input(|input| input.modifiers.ctrl);
    let step = if fine { 1 } else { TICKS_PER_BAR / 16 };
    let (fade, room) = if index == 0 {
        (
            &mut fades.fade_in,
            clip.length_ticks.saturating_sub(fades.fade_out.ticks),
        )
    } else {
        (
            &mut fades.fade_out,
            clip.length_ticks.saturating_sub(fades.fade_in.ticks),
        )
    };
    let mut edited = None;
    if dragging && let Some(pos) = controls::drag_position(ui, &response) {
        let tick = f64::from((pos.x - rect.left()) / tick_width);
        let ticks = if index == 0 {
            tick
        } else {
            f64::from(sound_end) - tick
        };
        let snapped = ((ticks / f64::from(step)).round() * f64::from(step)) as u32;
        edited = Some(snapped);
    }
    let mut direction = 0;
    if response.has_focus() {
        controls::keep_transport_key(ui.ctx(), response.id);
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    horizontal_arrows: true,
                    vertical_arrows: true,
                    ..Default::default()
                },
            );
        });
        ui.input_mut(|input| {
            let modifiers = input.modifiers;
            for key in [egui::Key::ArrowUp, egui::Key::ArrowRight] {
                direction += input.count_and_consume_key(modifiers, key) as i32;
            }
            for key in [egui::Key::ArrowDown, egui::Key::ArrowLeft] {
                direction -= input.count_and_consume_key(modifiers, key) as i32;
            }
            if input.consume_key(modifiers, egui::Key::Home) {
                edited = Some(0);
            }
            if input.consume_key(modifiers, egui::Key::End) {
                edited = Some(room);
            }
        });
        view::focus_outline(ui, &response, roles().accent);
    }
    ui.input(|input| {
        use egui::accesskit::{Action, ActionData};
        for request in input.accesskit_action_requests(response.id, Action::SetValue) {
            if let Some(ActionData::NumericValue(beats)) = request.data
                && beats.is_finite()
            {
                edited = Some((beats * f64::from(TICKS_PER_BEAT)).round() as u32);
            }
        }
        direction += input.num_accesskit_action_requests(response.id, Action::Increment) as i32
            - input.num_accesskit_action_requests(response.id, Action::Decrement) as i32;
    });
    if direction != 0 {
        edited = Some(
            edited
                .unwrap_or(fade.ticks)
                .saturating_add_signed(direction * step as i32),
        );
        if response.has_focus() {
            ui.memory_mut(|memory| memory.move_focus(egui::FocusDirection::None));
        }
    }
    if let Some(ticks) = edited {
        fade.ticks = ticks.min(room);
    }
    let beats = |ticks| f64::from(ticks) / f64::from(TICKS_PER_BEAT);
    response.widget_info(|| {
        let value = beats(fade.ticks);
        let mut info = egui::WidgetInfo::slider(response.enabled(), value, &name);
        info.current_text_value = Some(format!("{value:.3} beats"));
        info
    });
    ui.ctx().accesskit_node_builder(response.id, |node| {
        use egui::accesskit::Action;
        node.set_min_numeric_value(0.0);
        node.set_max_numeric_value(beats(room));
        node.set_numeric_value_step(beats(TICKS_PER_BAR / 16));
        node.add_action(Action::SetValue);
        node.add_action(Action::Increment);
        node.add_action(Action::Decrement);
    });
    response.hover_hint(format!(
        "Drag or use the arrow keys to set fade {edge} in sixteenths. \
         Hold Ctrl for single ticks."
    ));
}
