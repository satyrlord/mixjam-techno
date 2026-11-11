//! The lane header's own parts in the Arrange view: the derived lane name
//! and the short level fader. The song stores no lane names (decision A), so
//! a lane is named after the sound family of its first clip.
use super::mixer::{fraction_gain, gain_fraction, gain_keys, gain_semantics};
use super::*;
use mixjam_techno::application::LibraryView;

/// The lane header's width in the Arrange view.
pub(super) const LANE_HEADER_WIDTH: f32 = 250.0;
/// Where each part of the header sits, from its left edge: the track number,
/// the name's span, the M, S, L, and R switches' centres, and the fader's span.
pub(super) const NUMBER_X: f32 = 17.0;
pub(super) const NAME_SPAN: (f32, f32) = (34.0, 104.0);
pub(super) const SWITCH_X: [f32; 4] = [118.0, 142.0, 166.0, 190.0];
pub(super) const FADER_SPAN: (f32, f32) = (210.0, 244.0);

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

/// A short horizontal level fader for a lane: the same taper, keys, and
/// accessible name pattern as the Mixer's gain rail, named "{name} gain".
/// Returns whether the gain changed.
pub(super) fn lane_fader(ui: &mut egui::Ui, rect: Rect, name: &str, gain: &mut f32) -> bool {
    let before = *gain;
    let response = ui.interact(
        rect,
        ui.id().with(("lane-fader", name)),
        Sense::click_and_drag(),
    );
    let rail = rect.shrink2(Vec2::new(4.0, 0.0));
    if (response.clicked() || response.dragged())
        && let Some(pos) = response.interact_pointer_pos()
    {
        let fine = ui.input(|input| input.modifiers.shift);
        let fraction = if fine && response.dragged() {
            gain_fraction(*gain) + ui.input(|input| input.pointer.delta().x) / rail.width() * 0.1
        } else {
            (pos.x - rail.left()) / rail.width()
        };
        *gain = fraction_gain(fraction);
    }
    gain_keys(ui, &response, gain);
    let painter = ui.painter();
    let slot = Rect::from_center_size(rail.center(), Vec2::new(rail.width(), 4.0));
    painter.rect_filled(slot, 2.0, roles().canvas);
    let fraction = gain_fraction(*gain).clamp(0.0, 1.0);
    let x = egui::lerp(rail.left()..=rail.right(), fraction);
    painter.rect_filled(
        Rect::from_min_max(slot.min, Pos2::new(x, slot.bottom())),
        2.0,
        if response.hovered() || response.dragged() {
            roles().text
        } else {
            roles().muted
        },
    );
    // Unity gain is marked on the slot.
    let unity = egui::lerp(rail.left()..=rail.right(), gain_fraction(1.0));
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
    gain_semantics(ui, response, name, before, *gain);
    before != *gain
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
