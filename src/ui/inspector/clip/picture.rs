//! The picture at the top of a clip page: the clip at the panel's width with
//! its fades and gain line, and the fade handles that drag.
use super::*;
use mixjam_techno::application::Fades;

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
        let split = (handles[0].x + handles[1].x) / 2.0;
        let left = usize::from(handles[1].x < handles[0].x);
        let sound_end = view::sound_ticks(clip, sample, bpm);
        for (index, (edge, center)) in [controls::FadeEdge::In, controls::FadeEdge::Out]
            .into_iter()
            .zip(handles)
            .enumerate()
        {
            let name = format!("{CLIP} fade {} handle", edge.label());
            let (room, current) = if index == 0 {
                (
                    clip.length_ticks.saturating_sub(fades.fade_out.ticks),
                    fades.fade_in.ticks,
                )
            } else {
                (
                    clip.length_ticks.saturating_sub(fades.fade_in.ticks),
                    fades.fade_out.ticks,
                )
            };
            if let Some(ticks) = controls::fade_handle(
                ui,
                controls::FadeHandleSpec {
                    rect,
                    tick_width,
                    sound_end,
                    split,
                    is_left: index == left,
                    name: &name,
                    edge,
                    center,
                    room,
                },
                current,
            ) {
                if index == 0 {
                    fades.fade_in.ticks = ticks;
                } else {
                    fades.fade_out.ticks = ticks;
                }
            }
        }
    }
}
