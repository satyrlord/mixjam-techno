//! The lane header's own parts: the derived lane name and the small pan knob.
//! The song stores no lane names, so a lane is named after the sound family of
//! its first clip.
use super::mixer::{FaderRange, gain_keys, gain_semantics};
use super::*;
use mixjam_techno::application::LibraryView;

/// The lane header's width.
pub(super) const LANE_HEADER_WIDTH: f32 = 196.0;
/// Where each part of the header sits, from its left edge: the track number,
/// the name's span, the M and S switches' centres, and the pan knob's centre.
pub(super) const NUMBER_X: f32 = 17.0;
pub(super) const NAME_SPAN: (f32, f32) = (34.0, 114.0);
pub(super) const SWITCH_X: [f32; 2] = [128.0, 152.0];
pub(super) const PAN_X: f32 = 178.0;

/// A sound's family: its name without the number that tells it from its
/// siblings, so "deep chord 3" and "deep chord 12" are both "deep chord".
pub(super) fn sound_family(name: &str) -> &str {
    let name = name.trim();
    let family = name.trim_end_matches(|c: char| c.is_ascii_digit());
    if family.len() == name.len() {
        return name;
    }
    let family = family.trim_end_matches([' ', '-', '_', '.', '#']);
    if family.is_empty() { name } else { family }
}

/// The sound of each track's first clip, the earliest and then the oldest,
/// in one pass over the song.
pub(super) fn first_sounds(project: &Project) -> [Option<u32>; TRACK_COUNT] {
    let mut first: [Option<(u32, u64, u32)>; TRACK_COUNT] = [None; TRACK_COUNT];
    for clip in &project.clips {
        if let Some(slot) = first.get_mut(clip.track)
            && slot.is_none_or(|(tick, id, _)| (clip.start_tick, clip.id) < (tick, id))
        {
            *slot = Some((clip.start_tick, clip.id, clip.sample_id));
        }
    }
    first.map(|slot| slot.map(|(_, _, sample)| sample))
}

/// A lane's name: the sound family of its first clip's sound, else "Track N".
pub(super) fn lane_name(
    first_sound: Option<u32>,
    library: LibraryView<'_>,
    missing: impl Fn(u32) -> Option<String>,
    track: usize,
) -> String {
    first_sound
        .and_then(|id| {
            library
                .sample_by_id(id)
                .map(|sample| sample.name.clone())
                .or_else(|| missing(id))
        })
        .map(|name| sound_family(&name).to_owned())
        .unwrap_or_else(|| format!("Track {}", track + 1))
}

/// A small pan knob for an arrangement lane header: drag up/right or down/left,
/// double-click to center, or use arrow keys (Shift for fine steps), Home, and
/// End. An `automated` knob carries an outer ring (`U8`). The mouse wheel stays
/// free for vertical lane scrolling.
pub(super) fn lane_pan_knob(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    pan: &mut f32,
    automated: bool,
) -> bool {
    let before = *pan;
    let response = ui.interact(
        rect,
        ui.id().with(("lane-pan", name)),
        Sense::click_and_drag(),
    );
    pan_knob_input(ui, &response, pan);
    paint_pan_knob(ui, &response, rect, *pan, automated);
    pan_knob_semantics(ui, response, name, *pan, automated);
    before != *pan
}

fn pan_knob_input(ui: &egui::Ui, response: &Response, pan: &mut f32) {
    let fine = ui.input(|input| input.modifiers.shift);
    if controls::primary_dragged(ui, response) {
        let delta = ui.input(|input| input.pointer.delta());
        *pan += (delta.x - delta.y) / 90.0 * if fine { 0.1 } else { 1.0 };
    }
    if response.double_clicked() {
        *pan = 0.0;
    }
    if response.has_focus() {
        controls::range_knob_keys(
            ui,
            response.id,
            -1.0..=1.0,
            if fine { 0.01 } else { 0.05 },
            pan,
        );
    }
    controls::apply_numeric_accesskit(ui, response, pan, -1.0..=1.0);
    *pan = pan.clamp(-1.0, 1.0);
}

fn paint_pan_knob(ui: &egui::Ui, response: &Response, rect: Rect, pan: f32, automated: bool) {
    let center = rect.center();
    let radius = rect.width().min(rect.height()) / 2.0 - 1.5;
    let painter = ui.painter();
    painter.circle_filled(center, radius, roles().raised);
    let rim = if response.has_focus() {
        roles().accent
    } else if response.hovered() || response.dragged() {
        roles().muted
    } else {
        roles().line
    };
    painter.circle_stroke(center, radius, Stroke::new(1.0_f32, rim));
    // Mark 12 o'clock center so balanced pan reads immediately.
    painter.line_segment(
        [
            center - Vec2::new(0.0, radius - 0.5),
            center - Vec2::new(0.0, radius - 3.0),
        ],
        Stroke::new(1.0_f32, roles().muted.gamma_multiply(0.7)),
    );
    if pan.abs() > 0.005 {
        let arc: Vec<Pos2> = (0..=16)
            .map(|step| {
                let angle = (-90.0 + 135.0 * pan * step as f32 / 16.0).to_radians();
                center + Vec2::angled(angle) * (radius - 1.5)
            })
            .collect();
        painter.add(egui::Shape::line(arc, Stroke::new(1.5_f32, roles().accent)));
    }
    let angle = (-90.0 + 135.0 * pan).to_radians();
    let dir = Vec2::angled(angle);
    painter.line_segment(
        [center, center + dir * (radius - 2.5)],
        Stroke::new(1.5_f32, roles().text),
    );
    if automated {
        painter.circle_stroke(center, radius + 1.5, Stroke::new(1.0_f32, roles().legend));
    }
}

fn pan_knob_semantics(ui: &egui::Ui, response: Response, name: &str, pan: f32, automated: bool) {
    response.widget_info(|| {
        let mut info = egui::WidgetInfo::slider(response.enabled(), f64::from(pan), name);
        info.current_text_value = Some(view::pan_description(pan));
        info
    });
    controls::knob_accesskit_range(ui, &response, pan, -1.0..=1.0);
    response.hover_hint(format!(
        "{name}: {}. Drag or use arrow keys; Shift for fine steps; double-click to center.{}",
        view::pan_description(pan),
        if automated {
            "\nAutomated: its automation lane sets it while the song plays."
        } else {
            ""
        }
    ));
}

/// A clip gain fader reports explicit edits even at its current value. The
/// displayed value can stand for a selection with different saved gains.
pub(super) fn clip_fader(ui: &mut egui::Ui, rect: Rect, name: &str, gain: &mut f32) -> bool {
    let roles = roles();
    fader_in(
        ui,
        rect,
        name,
        gain,
        FaderRange::Clip,
        (roles.strip_well, roles.strip_fill, roles.strip_text),
    )
}

/// `(well, fill, active fill)` are the empty slot, the filled part at rest,
/// and the filled part while hovered or dragged.
fn fader_in(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    gain: &mut f32,
    range: FaderRange,
    (well, fill, active): (Color32, Color32, Color32),
) -> bool {
    let before = *gain;
    let response = ui.interact(
        rect,
        ui.id().with(("lane-fader", name)),
        Sense::click_and_drag(),
    );
    let rail = rect.shrink2(Vec2::new(4.0, 0.0));
    let dragged = controls::primary_dragged(ui, &response);
    let pointer = if dragged {
        controls::drag_position(ui, &response)
    } else {
        response
            .interact_pointer_pos()
            .filter(|_| controls::primary_clicked(ui, &response))
    };
    if let Some(pos) = pointer {
        let fine = ui.input(|input| input.modifiers.shift);
        let fraction = if fine && dragged {
            range.fraction(*gain) + ui.input(|input| input.pointer.delta().x) / rail.width() * 0.1
        } else {
            (pos.x - rail.left()) / rail.width()
        };
        *gain = range.gain(fraction);
    }
    let adjusted = gain_keys(ui, &response, gain, range);
    let painter = ui.painter();
    let slot = Rect::from_center_size(rail.center(), Vec2::new(rail.width(), 4.0));
    painter.rect_filled(slot, 2.0, well);
    let fraction = range.fraction(*gain);
    let x = egui::lerp(rail.left()..=rail.right(), fraction);
    painter.rect_filled(
        Rect::from_min_max(slot.min, Pos2::new(x, slot.bottom())),
        2.0,
        if response.hovered() || dragged {
            active
        } else {
            fill
        },
    );
    // Unity gain is marked on the slot.
    let unity = egui::lerp(rail.left()..=rail.right(), range.fraction(1.0));
    painter.line_segment(
        [
            Pos2::new(unity, slot.top() - 2.0),
            Pos2::new(unity, slot.bottom() + 2.0),
        ],
        Stroke::new(1.0_f32, roles().line),
    );
    let cap = Rect::from_center_size(
        Pos2::new(x, rail.center().y),
        Vec2::new(6.0, (rect.height() - 6.0).clamp(8.0, 14.0)),
    );
    painter.rect_filled(cap, 2.0, roles().cap);
    painter.rect_stroke(
        cap,
        2.0,
        Stroke::new(1.0_f32, roles().widget_edge),
        egui::StrokeKind::Inside,
    );
    if response.has_focus() {
        painter.rect_stroke(
            rect.shrink(1.0),
            2.0,
            Stroke::new(1.0_f32, roles().accent),
            egui::StrokeKind::Inside,
        );
    }
    gain_semantics(ui, response, name, before, *gain, range);
    before != *gain || matches!(range, FaderRange::Clip) && (pointer.is_some() || adjusted)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_family_drops_the_number_that_tells_siblings_apart() {
        assert_eq!(sound_family("good grasp 2"), "good grasp");
        assert_eq!(sound_family("deep chord 12"), "deep chord");
        assert_eq!(sound_family("Bass03"), "Bass");
        assert_eq!(sound_family("hat-7"), "hat");
        assert_eq!(sound_family("Kick"), "Kick");
        assert_eq!(sound_family("909"), "909");
    }

    #[test]
    fn a_family_preserves_punctuation_without_a_numeric_suffix() {
        assert_eq!(sound_family("HR16tech."), "HR16tech.");
        assert_eq!(sound_family("Kick-"), "Kick-");
        assert_eq!(sound_family("Snare_"), "Snare_");
        assert_eq!(sound_family("Bass#"), "Bass#");
    }

    #[test]
    fn a_family_ignores_outer_whitespace_before_reading_the_suffix() {
        assert_eq!(sound_family("deep chord 12 "), "deep chord");
        assert_eq!(sound_family("  hat-7\t"), "hat");
        assert_eq!(sound_family(" HR16tech. "), "HR16tech.");
        assert_eq!(sound_family(" 909 "), "909");
    }
}
