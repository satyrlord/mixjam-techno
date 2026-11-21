//! The album page of Album Review: the cover, the facts, the listening links,
//! and the track sections.
use super::chart::{energy_color, section_color, timeline_chart};
use super::grid::cover;
use super::*;
use mixjam_techno::application::album_review::{Kind, Review, Track};

pub(super) fn page(ui: &mut egui::Ui, state: &mut AlbumReviewState, id: &str) {
    let Some(review) = state.page_review(id) else {
        ui.label(format!(
            "The review \"{id}\" is no longer in the reviews folder."
        ));
        if controls::button(ui, "All albums", controls::ButtonRole::Primary).clicked() {
            state.page = None;
        }
        return;
    };
    let back = controls::button(ui, "← All albums", controls::ButtonRole::Secondary)
        .hover_hint("Back to the cover grid");
    if back.clicked() {
        state.page = None;
        return;
    }
    let top = back.rect;
    ui.horizontal_top(|ui| {
        ui.vertical(|ui| {
            ui.set_width(PAGE_COVER);
            let (rect, _) = ui.allocate_exact_size(Vec2::splat(PAGE_COVER), Sense::hover());
            cover(ui, state, id, rect);
            cover_controls(ui, state, &review);
        });
        ui.vertical(|ui| {
            ui.label(
                egui::RichText::new(&review.title)
                    .size(22.0)
                    .color(roles().legend),
            );
            ui.label(
                egui::RichText::new(&review.artist)
                    .size(15.0)
                    .color(roles().muted),
            );
            ui.add_space(4.0);
            egui::Grid::new("album-review-metadata")
                .num_columns(2)
                .spacing(Vec2::new(16.0, 4.0))
                .show(ui, |ui| {
                    let mut row = |name: &str, value: String| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(name).color(roles().muted))
                                .wrap_mode(egui::TextWrapMode::Extend),
                        );
                        ui.label(value);
                        ui.end_row();
                    };
                    row("Released", review.year.to_string());
                    row("Length", review.runtime.clone());
                    row("Tracks", review.tracks.len().to_string());
                    row("Label", review.label.clone());
                    row("Producer", review.producer.clone());
                    if !review.genre.trim().is_empty() {
                        row("Genre", review.genre.clone());
                    }
                    row("Genre tags", review.genre_tags.join(" · "));
                    if review.is_soundtrack == Some(true) {
                        row("Soundtrack", "Yes".into());
                    }
                    row(
                        "Review",
                        match review.kind {
                            Kind::Reference => "Reference album".into(),
                            Kind::Own => "Own work".into(),
                        },
                    );
                });
            listening(ui, &review);
        });
    });
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Overview")
            .size(16.0)
            .color(roles().legend),
    );
    for paragraph in review.overview.split("\n\n") {
        ui.label(paragraph.trim());
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Timeline")
            .size(16.0)
            .color(roles().legend),
    );
    if let Some(num) = timeline_chart(ui, &review) {
        state.reveal_track = Some(num);
    }
    ui.add_space(6.0);
    let reveal = state.reveal_track.take();
    for track in &review.tracks {
        track_section(ui, track, reveal == Some(track.num));
    }
    ui.add_space(6.0);
    ui.horizontal(|ui| {
        if controls::button(ui, "↑ Top", controls::ButtonRole::Secondary)
            .hover_hint("Back to the top of the page")
            .clicked()
        {
            ui.scroll_to_rect(top, Some(egui::Align::TOP));
        }
        if controls::button(ui, "All albums", controls::ButtonRole::Secondary).clicked() {
            state.page = None;
        }
    });
}

fn cover_controls(ui: &mut egui::Ui, state: &mut AlbumReviewState, review: &Review) {
    let loaded = state.textures.contains_key(&review.id);
    let pending = state
        .covers
        .as_ref()
        .is_some_and(|covers| covers.is_downloading(&review.id));
    match &review.cover_url {
        None => {
            ui.label("This review has no cover link.");
        }
        Some(url) if !loaded => {
            let response = ui
                .add_enabled_ui(!pending, |ui| {
                    controls::button(ui, "Display cover art", controls::ButtonRole::Primary)
                })
                .inner
                .hover_hint(format!("Download the cover from {url}"))
                .disabled_hint("The cover is downloading.");
            if response.clicked() {
                state.cover_errors.remove(&review.id);
                state.covers().request(&review.id, Some(url));
            }
            if pending {
                ui.label("Downloading the cover…");
            } else if let Some(reason) = state.cover_errors.get(&review.id) {
                ui.colored_label(roles().coral, format!("No cover: {reason}"));
            }
        }
        Some(_) => {}
    }
    if let Some(source) = &review.cover_source {
        ui.hyperlink_to("Cover source", source).hover_hint(source);
    }
}

fn listening(ui: &mut egui::Ui, review: &Review) {
    let links = [
        ("Listen on YouTube", &review.youtube_url),
        ("Listen on Bandcamp", &review.bandcamp_url),
    ];
    let local = [
        ("Audio file", &review.local_audio),
        ("Song file", &review.song_file),
    ];
    if links.iter().chain(&local).all(|(_, value)| value.is_none()) {
        ui.label(egui::RichText::new("No listening links.").color(roles().muted));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for (label, url) in links {
            if let Some(url) = url {
                ui.hyperlink_to(label, url).hover_hint(url);
            }
        }
    });
    for (label, path) in local {
        if let Some(path) = path {
            let shown = display_paths(path);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(label).color(roles().muted));
                ui.add(egui::Label::new(&shown).truncate())
                    .hover_hint(&shown);
            });
        }
    }
}

fn track_section(ui: &mut egui::Ui, track: &Track, reveal: bool) {
    ui.add_space(4.0);
    let heading = ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!("{}. {}", track.num, track.title))
                .size(15.0)
                .color(roles().legend),
        );
        ui.label(egui::RichText::new(&track.duration).color(roles().muted));
        ui.label(
            egui::RichText::new(format!("Energy: {}", track.energy.key()))
                .color(energy_color(track.energy))
                .strong(),
        );
        ui.label(egui::RichText::new(track.tags.join(" · ")).color(roles().muted));
    });
    if reveal {
        heading.response.scroll_to_me(Some(egui::Align::TOP));
    }
    ui.label(&track.role);
    for (label, path) in [
        ("Audio file", &track.local_audio),
        ("Song file", &track.song_file),
    ] {
        if let Some(path) = path {
            ui.label(format!("{label}: {}", display_paths(path)));
        }
    }
    // Each event: its time and section in a fixed column, then its text.
    const SECTION_WIDTH: f32 = 190.0;
    for event in &track.events {
        ui.horizontal_top(|ui| {
            ui.allocate_ui(Vec2::new(SECTION_WIDTH, 0.0), |ui| {
                ui.set_width(SECTION_WIDTH);
                ui.label(
                    egui::RichText::new(format!("{}  {}", event.timestamp, event.section))
                        .color(section_color(&event.section))
                        .strong(),
                );
            });
            ui.vertical(|ui| {
                ui.set_max_width(ui.available_width().max(240.0));
                ui.label(&event.description);
                if let Some(detail) = &event.detail {
                    ui.label(egui::RichText::new(detail).color(roles().muted).size(12.5));
                }
            });
        });
    }
    ui.separator();
}
