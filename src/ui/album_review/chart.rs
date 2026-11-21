//! The timeline chart of an album page, and the colors of its sections.
use super::*;
use mixjam_techno::application::album_review::{Energy, Review, clock_seconds, collection};

pub(super) fn energy_color(energy: Energy) -> Color32 {
    match energy {
        Energy::Low => Color32::from_rgb(137, 176, 210),
        Energy::Mid => Color32::from_rgb(162, 196, 133),
        Energy::High => Color32::from_rgb(232, 197, 133),
        Energy::Peak => roles().coral,
    }
}

/// A steady color for a section name, so that the same section reads the
/// same on every track.
pub(super) fn section_color(section: &str) -> Color32 {
    const COLORS: [Color32; 8] = [
        Color32::from_rgb(212, 193, 130),
        Color32::from_rgb(217, 157, 120),
        Color32::from_rgb(162, 196, 133),
        Color32::from_rgb(137, 176, 210),
        Color32::from_rgb(127, 190, 185),
        Color32::from_rgb(217, 145, 146),
        Color32::from_rgb(185, 158, 203),
        Color32::from_rgb(187, 179, 159),
    ];
    let key = collection::fold_key(section);
    let hash = key.bytes().fold(0u32, |hash, byte| {
        hash.wrapping_mul(31).wrapping_add(u32::from(byte))
    });
    COLORS[hash as usize % COLORS.len()]
}

/// One row per track: each section from its timestamp to the next, as wide
/// as its share of the longest track. Returns the track of a clicked row.
pub(super) fn timeline_chart(ui: &mut egui::Ui, review: &Review) -> Option<u32> {
    let longest = review
        .tracks
        .iter()
        .filter_map(|track| clock_seconds(&track.duration))
        .max()
        .unwrap_or(1)
        .max(1) as f32;
    let label_width = 210.0;
    let mut clicked = None;
    for track in &review.tracks {
        let width = (ui.available_width() - label_width - 60.0).max(120.0);
        let (row, response) =
            ui.allocate_exact_size(Vec2::new(label_width + width + 60.0, 20.0), Sense::click());
        let painter = ui.painter();
        let title = format!("{}. {}", track.num, track.title);
        let galley =
            painter.layout_no_wrap(title.clone(), FontId::proportional(12.0), roles().legend);
        painter
            .with_clip_rect(Rect::from_min_size(
                row.min,
                Vec2::new(label_width - 8.0, 20.0),
            ))
            .galley(
                Pos2::new(row.left(), row.center().y - galley.size().y / 2.0),
                galley,
                roles().legend,
            );
        let bar_left = row.left() + label_width;
        let duration = clock_seconds(&track.duration).unwrap_or(0) as f32;
        let x = |seconds: f32| bar_left + width * seconds / longest;
        let starts: Vec<f32> = track
            .events
            .iter()
            .map(|event| clock_seconds(&event.timestamp).unwrap_or(0) as f32)
            .collect();
        for (index, event) in track.events.iter().enumerate() {
            let start = starts[index];
            let end = starts.get(index + 1).copied().unwrap_or(duration);
            let segment = Rect::from_min_max(
                Pos2::new(x(start), row.top() + 3.0),
                Pos2::new(x(end).max(x(start) + 1.0), row.bottom() - 3.0),
            );
            painter.rect_filled(segment, 2.0, section_color(&event.section));
            painter.line_segment(
                [segment.left_top(), segment.left_bottom()],
                Stroke::new(1.0_f32, roles().ink),
            );
        }
        painter.text(
            Pos2::new(x(duration) + 6.0, row.center().y),
            Align2::LEFT_CENTER,
            &track.duration,
            FontId::proportional(11.0),
            roles().muted,
        );
        if response.hovered() {
            painter.rect_stroke(
                row.expand(1.0),
                2.0,
                Stroke::new(1.0_f32, roles().tools[0].1),
                egui::StrokeKind::Outside,
            );
        }
        let sections: Vec<String> = track
            .events
            .iter()
            .map(|event| format!("{} {}", event.timestamp, event.section))
            .collect();
        let name = format!("Timeline of track {}: {}", track.num, track.title);
        response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
        if response
            .hover_hint(format!("{title}\n{}", sections.join("\n")))
            .clicked()
        {
            clicked = Some(track.num);
        }
    }
    clicked
}
