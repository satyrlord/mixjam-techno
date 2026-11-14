//! The lane headers beside the score: the add row, the track number that arms
//! recording, the lane name that opens the Inspector, the switches, and the
//! level fader, or a mixer row in the Mix view.
use super::timeline::{HeaderChanges, ScoreFrame};
use super::*;

impl MixJamApp {
    /// Draw the header of one row and collect its changes.
    pub(super) fn lane_header(
        &mut self,
        ui: &mut egui::Ui,
        frame: &ScoreFrame,
        index: usize,
        first_sound: Option<u32>,
        headers: &mut HeaderChanges,
    ) {
        let (rect, _) = ui.allocate_exact_size(
            Vec2::new(frame.label_width, frame.row_height),
            Sense::hover(),
        );
        if frame.add_track == Some(index) {
            self.add_track_header(ui, rect, index);
            return;
        }
        if frame.mix {
            mixer::row_surface(ui, rect.shrink(1.0), false);
        } else {
            ui.painter()
                .rect_filled(rect.shrink(1.0), 4.0, roles().panel);
        }
        let project = self.session.project();
        let silenced = !project.tracks[index].audible(project.any_solo());
        self.lane_number(ui, frame, rect, index, silenced);
        if frame.mix {
            let (summary, detail) = headers
                .summaries
                .get_mut(index)
                .map(std::mem::take)
                .unwrap_or_default();
            let mut track = self.session.project().tracks[index].clone();
            let outcome = mix_view::track_row(
                ui,
                rect,
                frame.mix_offset,
                index,
                (&summary, &detail),
                frame.any_solo,
                &mut track,
            );
            if outcome.changed {
                headers.mix.tracks.push((index, track));
            }
            if outcome.open_eq {
                headers.mix.eq = Some(index);
            }
            return;
        }
        self.lane_name_button(ui, frame, rect, index, first_sound, silenced);
        self.lane_switches(ui, frame, rect, index, &mut headers.route);
        let (fader_left, fader_right) = lanes::FADER_SPAN;
        let mut gain = self.session.project().tracks[index].gain;
        if lanes::lane_fader(
            ui,
            Rect::from_x_y_ranges(
                rect.left() + fader_left..=rect.left() + fader_right,
                rect.y_range().shrink(1.0),
            ),
            &format!("Arrangement track {}", index + 1),
            &mut gain,
        ) {
            headers.gain = Some((index, gain));
        }
    }

    /// The header of the empty row, which selects its track for recording.
    fn add_track_header(&mut self, ui: &egui::Ui, rect: Rect, index: usize) {
        let response = ui.interact(rect, ui.id().with(("add_track", index)), Sense::click());
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::WidgetType::Button,
                ui.is_enabled(),
                format!("Add track {}", index + 1),
            )
        });
        if response.clicked() {
            self.recording.track = index;
            self.message(format!("Recording will create track {}.", index + 1));
        }
        if response.has_focus() {
            ui.painter().rect_stroke(
                rect.shrink(1.0),
                4.0,
                Stroke::new(1.0_f32, roles().accent),
                egui::StrokeKind::Inside,
            );
        }
        response.hover_hint("Drop a sound on this row or select it for recording.");
    }

    /// The track number, which selects the track for recording.
    fn lane_number(
        &mut self,
        ui: &egui::Ui,
        frame: &ScoreFrame,
        rect: Rect,
        index: usize,
        silenced: bool,
    ) {
        let row_height = frame.row_height;
        let (ink, quiet) = if frame.mix {
            (roles().strip_text, roles().strip_text.gamma_multiply(0.55))
        } else {
            (roles().text, roles().muted)
        };
        // From the left: the track number, the lane's name,
        // M, S, L, R, and the level fader.
        let number = Pos2::new(rect.left() + lanes::NUMBER_X, rect.center().y);
        if !frame.mix {
            ui.painter().rect_filled(
                Rect::from_center_size(number, Vec2::new(25.0, row_height - 3.0)),
                0.0,
                roles().panel,
            );
        }
        ui.painter().text(
            number + Vec2::new(0.0, -1.0),
            Align2::CENTER_CENTER,
            format!("{}", index + 1),
            FontId::proportional(14.0),
            if silenced { quiet } else { ink },
        );
        let arm = ui.interact(
            Rect::from_center_size(number, Vec2::new(26.0, row_height)),
            ui.id().with(("arm", index)),
            Sense::click(),
        );
        arm.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::RadioButton,
                ui.is_enabled(),
                self.recording.track == index,
                format!("Record on track {}", index + 1),
            )
        });
        if self.recording.track == index {
            ui.painter().circle_filled(
                number + Vec2::new(0.0, (row_height / 2.0 - 4.0).min(9.0)),
                2.0,
                roles().coral,
            );
        }
        if arm.has_focus() {
            ui.painter().rect_stroke(
                arm.rect.shrink2(Vec2::new(0.0, 2.0)),
                3.0,
                Stroke::new(1.0_f32, roles().accent),
                egui::StrokeKind::Inside,
            );
        }
        if arm.clicked() {
            self.recording.track = index;
            self.message(format!("Track {} is ready for recording.", index + 1));
        }
        arm.hover_hint("Select this track for recording");
    }

    /// The lane's name, which shows the track in the Inspector.
    fn lane_name_button(
        &mut self,
        ui: &egui::Ui,
        frame: &ScoreFrame,
        rect: Rect,
        index: usize,
        first_sound: Option<u32>,
        silenced: bool,
    ) {
        let name = lanes::lane_name(
            first_sound,
            self.session.library(),
            |id| {
                self.session
                    .missing_sample(id)
                    .map(|missing| missing.name.clone())
            },
            index,
        );
        let (name_left, name_right) = lanes::NAME_SPAN;
        // The name selects the track for the Inspector.
        let name_rect = Rect::from_x_y_ranges(
            rect.left() + name_left - 4.0..=rect.left() + name_right,
            rect.top() + 1.0..=rect.bottom() - 1.0,
        );
        let pick = ui.interact(
            name_rect,
            ui.id().with(("lane-name", index)),
            Sense::click(),
        );
        let picked = self.selection.track == Some(index);
        let pick_label = format!("Show track {} in the Inspector", index + 1);
        pick.widget_info(|| {
            egui::WidgetInfo::selected(
                egui::WidgetType::Button,
                ui.is_enabled(),
                picked,
                &pick_label,
            )
        });
        if picked || pick.hovered() || pick.has_focus() {
            ui.painter().rect_filled(
                name_rect,
                3.0,
                if picked {
                    roles().raised
                } else {
                    ui.visuals().widgets.hovered.weak_bg_fill
                },
            );
        }
        if picked {
            ui.painter().rect_filled(
                Rect::from_x_y_ranges(
                    name_rect.left()..=name_rect.left() + 2.0,
                    name_rect.y_range(),
                ),
                1.0,
                roles().accent,
            );
        }
        focus_outline(ui, &pick, roles().accent);
        if pick
            .hover_hint(
                "Show this track's level, pan, sends, and channel processing \
                 in the Inspector. The clip selection clears.",
            )
            .clicked()
        {
            self.select_track(index);
            if !self.panels.room.inspector {
                self.open_inspector();
            }
        }
        let name_size = if frame.row_height < 30.0 { 11.0 } else { 12.0 };
        let name_text = ui.painter().layout_job(elided(
            &name,
            name_size,
            if silenced {
                roles().muted
            } else {
                roles().text
            },
            name_right - name_left,
            1,
        ));
        ui.painter().galley(
            Pos2::new(
                rect.left() + name_left,
                rect.center().y - name_text.size().y / 2.0,
            ),
            name_text,
            roles().text,
        );
    }

    /// The track's switches, M, S, L, and R. A click is kept in `route` and
    /// applies after the score is drawn.
    fn lane_switches(
        &self,
        ui: &egui::Ui,
        frame: &ScoreFrame,
        rect: Rect,
        index: usize,
        route: &mut Option<(usize, TrackSwitch, bool)>,
    ) {
        let track = &self.session.project().tracks[index];
        for (switch, x) in TrackSwitch::ALL.into_iter().zip(lanes::SWITCH_X) {
            let button = Rect::from_center_size(
                Pos2::new(rect.left() + x, rect.center().y),
                Vec2::new(24.0, frame.row_height - 3.0),
            );
            let mut response = ui.interact(
                button,
                ui.id().with(("switch", index, switch)),
                Sense::click(),
            );
            let mut on = switch.get(track);
            if response.clicked() {
                on = !on;
                *route = Some((index, switch, on));
                response.mark_changed();
            }
            let label = format!("Arrangement track {} {}", index + 1, switch.name());
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, &label)
            });
            switch_letter(ui.painter(), button, on, switch);
            if response.has_focus() {
                ui.painter().rect_stroke(
                    button.shrink(1.0),
                    2.0,
                    Stroke::new(1.0_f32, roles().accent),
                    egui::StrokeKind::Inside,
                );
            }
            response.hover_hint(switch_hint(&label, switch, on));
        }
    }
}
