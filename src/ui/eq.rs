//! The 3-theme Channel Strip floating window (`736x500 pt`) for a single
//! track (`Track 1..64 EQ`): track switcher, live response/transfer/GR scopes,
//! and grouped hardware bays (`EQ`, `Drive`, `Compressor & Stereo`,
//! `Filter & Ducker`).
//! Edits flow back as `EditCommand::SetTrack`; the shared EQ curve is also
//! used by the Inspector.
use super::*;
use mixjam_techno::application::{
    TRACK_COUNT, TRACK_MONO_BELOW, TRACK_WIDTH, automation::FilterSlope, compressor::Compressor,
    drive::Drive, ducker::Ducker, eq::ChannelEq,
};

#[path = "eq/bays.rs"]
mod bays;
#[path = "eq/scopes.rs"]
mod scopes;

pub(super) use scopes::response_curve;

const CONTENT_W: f32 = 736.0;
const CONTENT_H: f32 = 500.0;
const HEADER_H: f32 = 26.0;
const SCOPE_H: f32 = 108.0;
const EQ_BAY_H: f32 = 124.0;
const ROW2_BAY_H: f32 = 220.0;
const GAP: f32 = 8.0;

/// Track settings edited by the Channel Strip window.
#[derive(Clone, Copy, PartialEq)]
struct Channel {
    eq: ChannelEq,
    drive: Drive,
    compressor: Compressor,
    width: f32,
    mono_below_hz: f32,
    filter_resonance: f32,
    filter_slope: FilterSlope,
    ducker: Ducker,
}

enum Action {
    None,
    Turn(Channel),
    Switch(Channel),
}

enum WindowEvent {
    Action(Action),
    SelectTrack(usize),
}

impl MixJamApp {
    /// Draw the Channel Strip window for `self.eq_track`, if open.
    pub(super) fn eq_window(&mut self, ctx: &egui::Context) {
        let Some(track) = self.eq_track else {
            return;
        };
        if track >= self.session.project().tracks.len() {
            self.eq_track = None;
            return;
        }
        let project = self.session.project();
        let current = &project.tracks[track];
        let channel = Channel {
            eq: current.eq,
            drive: current.drive,
            compressor: current.compressor,
            width: current.width,
            mono_below_hz: current.mono_below_hz,
            filter_resonance: current.filter_resonance,
            filter_slope: current.filter_slope,
            ducker: current.ducker,
        };
        let filter_active = !current.automation.filter.is_empty();
        let dynamics = scopes::Dynamics::read(&self.session, track);
        let generation = self.boards.master_view_generation;
        let name = format!("Track {} EQ", track + 1);
        let mut open = true;
        let mut event = WindowEvent::Action(Action::None);
        let viewport = ctx.viewport_rect().shrink(8.0);
        // Include the vertical scrollbar in the default width so the right
        // bay still fits in a short window.
        let scrollbar = egui::style::ScrollStyle::solid().allocated_width();

        dialog::window(&name)
            .open(&mut open)
            .palette(controls::Palette::Utility)
            .default_width(CONTENT_W + 36.0 + scrollbar)
            .default_height(CONTENT_H + 20.0)
            .constrain_to(viewport)
            .show(ctx, |ui| {
                event = ui
                    .push_id(generation, |ui| {
                        show_strip(ui, &name, track, &channel, dynamics, filter_active)
                    })
                    .inner;
            });

        if !open {
            self.eq_track = None;
            return;
        }
        match event {
            WindowEvent::SelectTrack(next) => {
                let next = next.min(TRACK_COUNT - 1);
                // The title names the track, so keep the window where it is.
                dialog::carry_geometry(ctx, &name, &format!("Track {} EQ", next + 1));
                self.eq_track = Some(next);
                ctx.request_repaint();
            }
            WindowEvent::Action(Action::None) => {}
            WindowEvent::Action(Action::Turn(updated) | Action::Switch(updated)) => {
                let discrete = matches!(event, WindowEvent::Action(Action::Switch(_)));
                self.apply_channel_edit(track, updated, discrete);
                ctx.request_repaint();
            }
        }
    }

    fn apply_channel_edit(&mut self, track: usize, updated: Channel, discrete: bool) {
        let mut edited = self.session.project().tracks[track].clone();
        edited.eq = updated.eq;
        edited.drive = updated.drive;
        edited.compressor = updated.compressor;
        edited.width = updated.width;
        edited.mono_below_hz = updated.mono_below_hz;
        edited.filter_resonance = updated.filter_resonance;
        edited.filter_slope = updated.filter_slope;
        edited.ducker = updated.ducker;
        let command = EditCommand::SetTrack {
            index: track,
            track: Box::new(edited),
        };
        let result = if discrete {
            self.session.edit_discrete(command)
        } else {
            self.session.edit(command)
        };
        self.apply(result);
    }
}

fn show_strip(
    ui: &mut egui::Ui,
    name: &str,
    track: usize,
    channel: &Channel,
    dynamics: scopes::Dynamics,
    filter_active: bool,
) -> WindowEvent {
    let mut draft = *channel;
    let (bounds, _) = ui.allocate_exact_size(Vec2::new(CONTENT_W, CONTENT_H), Sense::hover());
    let ui = &mut ui.new_child(egui::UiBuilder::new().max_rect(bounds));

    let header_rect = Rect::from_min_size(bounds.min, Vec2::new(CONTENT_W, HEADER_H));
    // Paint the whole body even on a header click, so the frame is not blank.
    let header_event = header_bar(ui, header_rect, name, track, &mut draft, filter_active);

    let scope_y = header_rect.bottom() + 6.0;
    let eq_scope =
        Rect::from_min_size(Pos2::new(bounds.left(), scope_y), Vec2::new(328.0, SCOPE_H));
    let drv_scope = Rect::from_min_size(
        Pos2::new(eq_scope.right() + GAP, scope_y),
        Vec2::new(192.0, SCOPE_H),
    );
    let gr_scope = Rect::from_min_size(
        Pos2::new(drv_scope.right() + GAP, scope_y),
        Vec2::new(200.0, SCOPE_H),
    );
    scopes::response_curve(ui, eq_scope, &draft.eq, name);
    scopes::drive_scope(ui, drv_scope, &draft.drive, track);
    dynamics.show(ui, gr_scope, track);

    let mut flags = bays::EditFlags {
        turned: false,
        switched: false,
    };
    let eq_bay_rect = Rect::from_min_size(
        Pos2::new(bounds.left(), eq_scope.bottom() + GAP),
        Vec2::new(CONTENT_W, EQ_BAY_H),
    );
    flags.merge(bays::eq_bay(ui, eq_bay_rect, name, &mut draft));

    let row2_y = eq_bay_rect.bottom() + GAP;
    let drv_rect = Rect::from_min_size(
        Pos2::new(bounds.left(), row2_y),
        Vec2::new(188.0, ROW2_BAY_H),
    );
    let cmp_rect = Rect::from_min_size(
        Pos2::new(drv_rect.right() + GAP, row2_y),
        Vec2::new(280.0, ROW2_BAY_H),
    );
    let duk_rect = Rect::from_min_size(
        Pos2::new(cmp_rect.right() + GAP, row2_y),
        Vec2::new(252.0, ROW2_BAY_H),
    );
    if theme::roles().material == theme::Material::Metal {
        paint_modular_routing_traces(ui.painter(), eq_bay_rect, drv_rect, cmp_rect, duk_rect);
    }

    let track_num = track + 1;
    flags.merge(bays::drive_bay(ui, drv_rect, track_num, &mut draft));
    flags.merge(bays::comp_stereo_bay(ui, cmp_rect, track_num, &mut draft));
    flags.merge(bays::filter_ducker_bay(ui, duk_rect, track, &mut draft));

    if let Some(header_event) = header_event {
        header_event
    } else if flags.switched {
        WindowEvent::Action(Action::Switch(draft))
    } else if flags.turned {
        WindowEvent::Action(Action::Turn(draft))
    } else {
        WindowEvent::Action(Action::None)
    }
}

fn paint_modular_routing_traces(
    painter: &egui::Painter,
    eq_bay: Rect,
    drv: Rect,
    cmp: Rect,
    duk: Rect,
) {
    let trace = Stroke::new(1.2_f32, Color32::from_rgb(218, 118, 44));
    let drop_y = eq_bay.bottom() + 4.0;
    let eq_out = Pos2::new(eq_bay.center().x, eq_bay.bottom());
    painter.line_segment([eq_out, Pos2::new(eq_out.x, drop_y)], trace);
    let left_bus = Pos2::new(drv.center().x, drop_y);
    let right_bus = Pos2::new(duk.center().x, drop_y);
    painter.line_segment([left_bus, right_bus], trace);
    for target in [drv, cmp, duk] {
        let tx = target.center().x;
        painter.line_segment([Pos2::new(tx, drop_y), Pos2::new(tx, target.top())], trace);
        painter.circle_filled(Pos2::new(tx, drop_y), 1.8, trace.color);
    }
}

/// The header lamps: each section tag, whether the section shapes the sound,
/// and its lamp color.
fn header_leds(draft: &Channel, filter_active: bool) -> [(&'static str, bool, egui::Color32); 6] {
    let r = roles();
    let stereo_active =
        draft.width != TRACK_WIDTH.default || draft.mono_below_hz > TRACK_MONO_BELOW.low + 0.5;
    let filter_shaped =
        filter_active || draft.filter_resonance > 0.0 || draft.filter_slope != FilterSlope::Slope12;
    [
        ("EQ", draft.eq.on, r.sections[1]),
        ("DRV", draft.drive.on, r.sections[5]),
        ("CMP", draft.compressor.on, r.sections[4]),
        ("WID", stereo_active, r.sections[0]),
        ("FLT", filter_shaped, r.sections[3]),
        ("DUK", draft.ducker.on, r.sections[2]),
    ]
}

#[cfg(test)]
#[test]
fn the_filter_lamp_lights_for_the_lane_resonance_and_slope() {
    let flat = Channel {
        eq: ChannelEq::default(),
        drive: Drive::default(),
        compressor: Compressor::default(),
        width: TRACK_WIDTH.default,
        mono_below_hz: TRACK_MONO_BELOW.low,
        filter_resonance: 0.0,
        filter_slope: FilterSlope::Slope12,
        ducker: Ducker::default(),
    };
    let lamp = |channel: Channel, automated: bool| header_leds(&channel, automated)[4];
    assert_eq!(lamp(flat, false).0, "FLT");
    assert!(!lamp(flat, false).1, "a flat filter lane rests");
    assert!(lamp(flat, true).1, "an automated cutoff lights the lamp");
    let resonant = Channel {
        filter_resonance: 30.0,
        ..flat
    };
    assert!(lamp(resonant, false).1, "resonance lights the lamp");
    let steep = Channel {
        filter_slope: FilterSlope::Slope24,
        ..flat
    };
    assert!(lamp(steep, false).1, "the 24 dB slope lights the lamp");
    assert!(header_leds(&flat, false).iter().all(|(_, on, _)| !on));
}

fn header_bar(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    track: usize,
    draft: &mut Channel,
    filter_active: bool,
) -> Option<WindowEvent> {
    let prev_rect = Rect::from_min_size(rect.min, Vec2::new(26.0, 24.0));
    let prev_name = format!("{name} previous track");
    let prev_resp = controls::button_at(
        ui,
        prev_rect,
        &prev_name,
        "<",
        controls::ButtonRole::Secondary,
        track > 0,
    );
    if name_control(prev_resp, &prev_name)
        .hover_hint("Switch Channel Strip to the previous track.")
        .clicked()
    {
        return Some(WindowEvent::SelectTrack(track - 1));
    }

    let badge_rect = Rect::from_min_size(
        Pos2::new(prev_rect.right() + 4.0, rect.top()),
        Vec2::new(68.0, 24.0),
    );
    controls::well(ui, badge_rect);
    ui.painter().text(
        badge_rect.center(),
        Align2::CENTER_CENTER,
        format!("Track {}", track + 1),
        FontId::proportional(11.0),
        roles().legend,
    );

    let next_rect = Rect::from_min_size(
        Pos2::new(badge_rect.right() + 4.0, rect.top()),
        Vec2::new(26.0, 24.0),
    );
    let next_name = format!("{name} next track");
    let next_resp = controls::button_at(
        ui,
        next_rect,
        &next_name,
        ">",
        controls::ButtonRole::Secondary,
        track + 1 < TRACK_COUNT,
    );
    if name_control(next_resp, &next_name)
        .hover_hint("Switch Channel Strip to the next track.")
        .clicked()
    {
        return Some(WindowEvent::SelectTrack(track + 1));
    }

    let r = roles();
    let led_x0 = next_rect.right() + 14.0;
    for (idx, (tag, on, color)) in header_leds(draft, filter_active).into_iter().enumerate() {
        let chip = Rect::from_min_size(
            Pos2::new(led_x0 + idx as f32 * 44.0, rect.top() + 2.0),
            Vec2::new(40.0, 20.0),
        );
        ui.painter().rect_filled(chip, 3.0, r.strip_slot);
        ui.painter().circle_filled(
            Pos2::new(chip.left() + 7.0, chip.center().y),
            3.0,
            if on { color } else { r.lamp_off },
        );
        ui.painter().text(
            Pos2::new(chip.left() + 14.0, chip.center().y),
            Align2::LEFT_CENTER,
            tag,
            FontId::proportional(9.5),
            if on { r.legend } else { r.disabled_text },
        );
    }

    let reset_name = format!("Reset {name}");
    let reset_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 80.0, rect.top()),
        Vec2::new(80.0, 24.0),
    );
    let reset_resp = controls::button_at(
        ui,
        reset_rect,
        &reset_name,
        "Reset",
        controls::ButtonRole::Secondary,
        true,
    );
    if name_control(reset_resp, &reset_name)
        .hover_hint("Return every band to flat. The power switch stays as it is.")
        .clicked()
    {
        draft.eq = ChannelEq {
            on: draft.eq.on,
            ..ChannelEq::default()
        };
        return Some(WindowEvent::Action(Action::Switch(*draft)));
    }
    None
}
