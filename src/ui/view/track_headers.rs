//! The lane headers beside the score: the add row, the track number that arms
//! recording, the lane name that opens the Inspector, Mute and Solo squares,
//! and a small pan knob.
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
        ui.painter()
            .rect_filled(rect.shrink(1.0), 4.0, roles().panel);
        let project = self.session.project();
        let track = &project.tracks[index];
        let silenced = !track.sounds(project.any_solo());
        let pan_automated = !track.automation.pan.is_empty();
        let mut pan = track.pan;
        self.lane_number(ui, frame, rect, index, silenced);
        self.lane_name_button(ui, frame, rect, index, first_sound, silenced);
        self.lane_switches(ui, frame, rect, index, &mut headers.routes);
        let knob = (frame.row_height - 4.0).clamp(16.0, 22.0);
        if lanes::lane_pan_knob(
            ui,
            Rect::from_center_size(
                Pos2::new(rect.left() + lanes::PAN_X, rect.center().y),
                Vec2::splat(knob),
            ),
            &format!("Arrangement track {} pan", index + 1),
            &mut pan,
            pan_automated,
        ) {
            headers.pans.push((index, pan));
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
        let (ink, quiet) = (roles().text, roles().muted);
        // From the left: the track number, the lane's name, M, S, and pan.
        let number = Pos2::new(rect.left() + lanes::NUMBER_X, rect.center().y);
        ui.painter().rect_filled(
            Rect::from_center_size(number, Vec2::new(25.0, row_height - 3.0)),
            0.0,
            roles().panel,
        );
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
        let picked = self.inspector_page() == inspector::Page::Track(index);
        let show_song = picked && self.panels.room.inspector;
        if lane_name_box(ui, index, name_rect, picked, show_song) {
            if show_song {
                // A second click returns the Inspector to the song.
                self.selection.track = None;
            } else {
                self.select_track(index);
                if !self.panels.room.inspector {
                    self.open_inspector();
                }
            }
        }
        paint_lane_name_text(
            ui,
            Rect::from_x_y_ranges(
                rect.left() + name_left..=rect.left() + name_right,
                rect.y_range(),
            ),
            &name,
            frame.row_height,
            silenced,
        );
    }

    /// The track's Mute and Solo squares. Each click is kept in `routes` and
    /// applies after the score is drawn.
    fn lane_switches(
        &self,
        ui: &egui::Ui,
        frame: &ScoreFrame,
        rect: Rect,
        index: usize,
        routes: &mut Vec<(usize, TrackSwitch, bool)>,
    ) {
        let track = &self.session.project().tracks[index];
        let side = (frame.row_height - 6.0).clamp(14.0, 20.0);
        for (switch, x) in [TrackSwitch::Mute, TrackSwitch::Solo]
            .into_iter()
            .zip(lanes::SWITCH_X)
        {
            let button = Rect::from_center_size(
                Pos2::new(rect.left() + x, rect.center().y),
                Vec2::splat(side),
            );
            let mut response = ui.interact(
                button,
                ui.id().with(("switch", index, switch)),
                Sense::click(),
            );
            let mut on = switch.get(track);
            if response.clicked() {
                on = !on;
                routes.push((index, switch, on));
                response.mark_changed();
            }
            let label = format!("Arrangement track {} {}", index + 1, switch.name());
            response.widget_info(|| {
                egui::WidgetInfo::selected(egui::WidgetType::Checkbox, ui.is_enabled(), on, &label)
            });
            paint_lane_switch(ui.painter(), button, on, response.hovered(), switch);
            if response.has_focus() {
                ui.painter().rect_stroke(
                    button.shrink(1.0),
                    2.5,
                    Stroke::new(1.0_f32, roles().accent),
                    egui::StrokeKind::Inside,
                );
            }
            response.hover_hint(switch_hint(&label, switch, on));
        }
    }
}

fn lane_name_box(
    ui: &egui::Ui,
    index: usize,
    name_rect: Rect,
    picked: bool,
    show_song: bool,
) -> bool {
    let pick = ui.interact(
        name_rect,
        ui.id().with(("lane-name", index)),
        Sense::click(),
    );
    let pick_label = if show_song {
        "Show the song in the Inspector".to_owned()
    } else {
        format!("Show track {} in the Inspector", index + 1)
    };
    pick.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::Button,
            ui.is_enabled(),
            picked,
            &pick_label,
        )
    });
    if picked {
        ui.painter().rect_filled(name_rect, 3.0, roles().raised);
        ui.painter().rect_filled(
            Rect::from_x_y_ranges(
                name_rect.left()..=name_rect.left() + 2.0,
                name_rect.y_range(),
            ),
            1.0,
            roles().accent,
        );
    } else if pick.hovered() || pick.has_focus() {
        ui.painter()
            .rect_filled(name_rect, 3.0, ui.visuals().widgets.hovered.weak_bg_fill);
    }
    focus_outline(ui, &pick, roles().accent);
    pick.hover_hint(if show_song {
        "The Inspector shows this track. Click again to show the song."
    } else {
        "Show this track's channel processing in the Inspector. The clip selection clears."
    })
    .clicked()
}

fn paint_lane_name_text(ui: &egui::Ui, rect: Rect, name: &str, row_height: f32, silenced: bool) {
    let size = if row_height < 30.0 { 11.0 } else { 12.0 };
    let color = if silenced {
        roles().muted
    } else {
        roles().text
    };
    let galley = ui
        .painter()
        .layout_job(elided(name, size, color, rect.width(), 1));
    ui.painter().galley(
        Pos2::new(rect.left(), rect.center().y - galley.size().y / 2.0),
        galley,
        roles().text,
    );
}

fn paint_lane_switch(
    painter: &egui::Painter,
    rect: Rect,
    on: bool,
    hovered: bool,
    switch: TrackSwitch,
) {
    let (fill, letter) = match switch {
        TrackSwitch::Mute if on => (roles().coral, roles().on_light),
        TrackSwitch::Solo if on => (roles().accent, roles().on_light),
        _ => (
            if hovered {
                roles().hover
            } else {
                roles().raised
            },
            roles().muted,
        ),
    };
    painter.rect_filled(rect, 2.5, fill);
    if !on {
        painter.rect_stroke(
            rect,
            2.5,
            Stroke::new(1.0_f32, roles().line),
            egui::StrokeKind::Inside,
        );
    }
    painter.text(
        rect.center(),
        Align2::CENTER_CENTER,
        switch.letter(),
        FontId::proportional(10.0),
        letter,
    );
}
