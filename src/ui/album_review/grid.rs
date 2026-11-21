//! The cover grid of Album Review: search, filters, cards, and the footer.
use super::state::GridCache;
use super::*;
use mixjam_techno::application::album_review::{
    Kind, Review, Shelf,
    collection::{self, Sort, TOP_GENRES},
    covers::CACHE_LIMIT_BYTES,
};

pub(super) fn grid(ui: &mut egui::Ui, state: &mut AlbumReviewState) {
    state.shelf();
    // The grid reads the reviews while it changes the view state; the
    // shelf returns to the state when the grid is drawn.
    let Some(shelf) = state.shelf.take() else {
        return;
    };
    grid_body(ui, state, &shelf);
    state.shelf = Some(shelf);
    // The footer asks for a reload while the grid holds the shelf.
    if std::mem::take(&mut state.reload) {
        let shelf = state.shelf();
        if shelf.folder().is_none() {
            *shelf = Shelf::open_default();
        } else {
            shelf.reload();
        }
        let count = shelf.reviews().len();
        state.grid = GridCache::default();
        state.cache_message = Some(format!(
            "Read {count} review{} again.",
            if count == 1 { "" } else { "s" }
        ));
    }
}

fn grid_body(ui: &mut egui::Ui, state: &mut AlbumReviewState, shelf: &Shelf) {
    if shelf.folder().is_none() {
        ui.label(egui::RichText::new("No reviews folder").size(17.0).strong());
        ui.label(
            "The application looks for the reviews in assets/album-review beside its own \
             folder: the repository root for a build, or the installation folder. Restore \
             that folder, then select Reload reviews.",
        );
        if controls::button(ui, "Reload reviews", controls::ButtonRole::Primary).clicked() {
            state.reload = true;
        }
        return;
    }
    let problems = shelf.problems();
    let reviews = shelf.reviews();
    if reviews.is_empty() {
        ui.label(egui::RichText::new("No albums available.").size(15.0));
        problems_list(ui, problems);
        footer(ui, state);
        return;
    }
    state
        .grid
        .update(shelf.revision(), reviews, &state.filter, state.sort);
    let visible = state.grid.visible.clone();
    ui.horizontal(|ui| {
        let search = ui.add(
            egui::TextEdit::singleline(&mut state.filter.search)
                .hint_text("Search title, artist, or genre")
                .desired_width((ui.available_width() - 330.0).clamp(160.0, 420.0)),
        );
        name_control(search, "Search albums");
        let sort = egui::ComboBox::from_id_salt("album-review-sort")
            .selected_text(state.sort.label())
            .width(130.0)
            .show_ui(ui, |ui| {
                for sort in Sort::ALL {
                    ui.selectable_value(&mut state.sort, sort, sort.label());
                }
            })
            .response;
        name_control(sort, "Sort albums");
        let count = ui.label(collection::count_label(visible.len(), reviews.len()));
        name_control(count, "Album count");
    });
    filter_rows(ui, state, reviews, shelf.revision(), !visible.is_empty());
    if visible.is_empty() {
        ui.add_space(8.0);
        ui.label("No results for the current search and filters.");
        if controls::button(ui, "Clear filters", controls::ButtonRole::Primary).clicked() {
            state.filter.clear();
        }
    } else {
        ui.add_space(4.0);
        ui.horizontal_wrapped(|ui| {
            ui.spacing_mut().item_spacing = Vec2::new(10.0, 10.0);
            for index in visible {
                let review = &reviews[index];
                if !state.checked.contains(&review.id) {
                    state.checked.insert(review.id.clone());
                    state.covers().request(&review.id, None);
                }
                if card(ui, state, review).clicked() {
                    state.page = Some(review.id.clone());
                }
            }
        });
    }
    problems_list(ui, problems);
    footer(ui, state);
}

/// The width of a `controls::button` with this label.
fn button_width(ui: &egui::Ui, label: &str) -> f32 {
    let text = ui
        .painter()
        .layout_no_wrap(label.into(), FontId::proportional(13.0), roles().ink)
        .size()
        .x;
    (text + 26.0).max(72.0)
}

fn chip(ui: &mut egui::Ui, label: &str, active: bool, name: &str) -> Response {
    let role = if active {
        controls::ButtonRole::Primary
    } else {
        controls::ButtonRole::Secondary
    };
    let response = controls::button(ui, label, role);
    response.widget_info(|| {
        egui::WidgetInfo::selected(egui::WidgetType::Button, response.enabled(), active, name)
    });
    response
}

/// The artist and genre rows, and the chips of the active filters. Without
/// results, the empty state carries the one Clear filters button.
fn filter_rows(
    ui: &mut egui::Ui,
    state: &mut AlbumReviewState,
    reviews: &[Review],
    revision: u64,
    results: bool,
) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Artists")
                .color(roles().legend)
                .strong(),
        );
        for artist in &state.grid.artists {
            let active = state.filter.artists.contains(artist);
            if chip(ui, artist, active, &format!("Artist filter: {artist}")).clicked()
                && !state.filter.artists.remove(artist)
            {
                state.filter.artists.insert(artist.clone());
            }
        }
    });
    // An artist chosen above changes which genres still find an album.
    state
        .grid
        .update(revision, reviews, &state.filter, state.sort);
    let genres = &state.grid.genres;
    let hidden = genres.len().saturating_sub(TOP_GENRES);
    ui.horizontal_wrapped(|ui| {
        ui.label(egui::RichText::new("Genres").color(roles().legend).strong());
        for (index, (genre, count)) in genres.iter().enumerate() {
            let active = state.filter.genres.contains(genre);
            if !state.genres_expanded && index >= TOP_GENRES && !active {
                continue;
            }
            let usable = active || state.grid.has_results[index];
            let label = format!("{genre} ({count})");
            // A disabled chip draws in its own scope, which does not wrap by
            // itself; start a new row when it would not fit.
            if ui.available_size_before_wrap().x < button_width(ui, &label) {
                ui.end_row();
            }
            let response = ui
                .add_enabled_ui(usable, |ui| {
                    chip(ui, &label, active, &format!("Genre filter: {genre}"))
                })
                .inner
                .disabled_hint("No album has this tag together with the selection.");
            if response.clicked() && !state.filter.genres.remove(genre) {
                state.filter.genres.insert(genre.clone());
            }
        }
        if hidden > 0 {
            let label = if state.genres_expanded {
                "Show fewer".to_string()
            } else {
                format!("+{hidden} more")
            };
            if controls::button(ui, &label, controls::ButtonRole::Secondary).clicked() {
                state.genres_expanded = !state.genres_expanded;
            }
        }
    });
    if state.filter.is_empty() {
        return;
    }
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Filters")
                .color(roles().legend)
                .strong(),
        );
        let mut remove_artist = None;
        let mut remove_genre = None;
        for artist in &state.filter.artists {
            let chip =
                controls::button(ui, &format!("{artist} ×"), controls::ButtonRole::Secondary);
            if name_control(chip, &format!("Remove filter: {artist}")).clicked() {
                remove_artist = Some(artist.clone());
            }
        }
        for genre in &state.filter.genres {
            let chip = controls::button(ui, &format!("{genre} ×"), controls::ButtonRole::Secondary);
            if name_control(chip, &format!("Remove filter: {genre}")).clicked() {
                remove_genre = Some(genre.clone());
            }
        }
        if let Some(artist) = remove_artist {
            state.filter.artists.remove(&artist);
        }
        if let Some(genre) = remove_genre {
            state.filter.genres.remove(&genre);
        }
        if results
            && controls::button(ui, "Clear filters", controls::ButtonRole::Secondary).clicked()
        {
            state.filter.clear();
        }
    });
}

/// A placeholder for a cover that is not loaded: a record sleeve outline.
fn placeholder(ui: &egui::Ui, rect: Rect) {
    let painter = ui.painter();
    painter.rect_filled(rect, 4.0, roles().ink);
    let disc = rect.center() + Vec2::new(rect.width() * 0.12, 0.0);
    let radius = rect.width() * 0.3;
    painter.circle_stroke(
        disc,
        radius,
        Stroke::new(2.0_f32, roles().muted.gamma_multiply(0.5)),
    );
    painter.circle_filled(disc, radius * 0.35, roles().muted.gamma_multiply(0.35));
    let sleeve = Rect::from_center_size(
        rect.center() - Vec2::new(rect.width() * 0.12, 0.0),
        Vec2::splat(rect.width() * 0.56),
    );
    painter.rect_filled(sleeve, 3.0, roles().ink);
    painter.rect_stroke(
        sleeve,
        3.0,
        Stroke::new(2.0_f32, roles().muted.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
}

pub(super) fn cover(ui: &egui::Ui, state: &AlbumReviewState, id: &str, rect: Rect) {
    match state.textures.get(id) {
        Some(texture) => {
            let size = texture.size_vec2();
            let scale = (rect.width() / size.x).min(rect.height() / size.y);
            let shown = Rect::from_center_size(rect.center(), size * scale);
            ui.painter().rect_filled(rect, 4.0, roles().ink);
            ui.painter().image(
                texture.id(),
                shown,
                Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                Color32::WHITE,
            );
        }
        None => placeholder(ui, rect),
    }
}

fn card(ui: &mut egui::Ui, state: &AlbumReviewState, review: &Review) -> Response {
    let (rect, response) = ui.allocate_exact_size(CARD, Sense::click());
    let hovered = response.hovered() || response.has_focus();
    ui.painter().rect_filled(
        rect,
        5.0,
        if hovered {
            roles().face.gamma_multiply(1.4)
        } else {
            roles().face
        },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            5.0,
            Stroke::new(2.0_f32, roles().tools[0].1),
            egui::StrokeKind::Inside,
        );
    }
    let art = Rect::from_min_size(
        rect.min + Vec2::new((CARD.x - COVER) / 2.0, 6.0),
        Vec2::splat(COVER),
    );
    cover(ui, state, &review.id, art);
    let text = |y: f32, value: &str, size: f32, color: Color32| {
        let galley = ui.painter().layout(
            value.to_owned(),
            FontId::proportional(size),
            color,
            CARD.x - 12.0,
        );
        let row = galley.rows.first().map_or(0.0, |row| row.height());
        let clipped = Rect::from_min_size(
            Pos2::new(rect.left() + 6.0, rect.top() + y),
            Vec2::new(CARD.x - 12.0, row),
        );
        ui.painter()
            .with_clip_rect(clipped)
            .galley(clipped.min, galley, color);
    };
    text(150.0, &review.artist.to_uppercase(), 11.0, roles().muted);
    text(166.0, &review.title, 13.5, roles().legend);
    let detail = format!(
        "{} · {} track{}{}",
        review.year,
        review.tracks.len(),
        if review.tracks.len() == 1 { "" } else { "s" },
        if review.kind == Kind::Own {
            " · Own work"
        } else {
            ""
        }
    );
    text(188.0, &detail, 11.5, roles().muted);
    let name = format!("Open review: {} — {}", review.artist, review.title);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    response.hover_hint(format!(
        "{} — {} ({})\n{}",
        review.artist,
        review.title,
        review.year,
        review.genre_tags.join(" · ")
    ))
}

fn problems_list(ui: &mut egui::Ui, problems: &[(String, String)]) {
    if problems.is_empty() {
        return;
    }
    controls::disclosure(format!("Files that did not load ({})", problems.len())).show(ui, |ui| {
        for (file, error) in problems {
            ui.colored_label(roles().coral, format!("{file}: {error}"));
        }
    });
}

fn footer(ui: &mut egui::Ui, state: &mut AlbumReviewState) {
    ui.add_space(6.0);
    ui.separator();
    let (count, bytes) = state.cache_usage();
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "Cover cache: {count} cover{}, {:.1} MB of {} MB.",
            if count == 1 { "" } else { "s" },
            bytes as f64 / 1024.0 / 1024.0,
            CACHE_LIMIT_BYTES / 1024 / 1024
        ));
        let busy = state.covers().busy();
        if ui
            .add_enabled_ui(!busy, |ui| {
                controls::button(ui, "Clear cover cache", controls::ButtonRole::Secondary)
            })
            .inner
            .hover_hint("Remove every downloaded cover. Each cover downloads again on request.")
            .disabled_hint("Wait for cover loading to finish.")
            .clicked()
        {
            state.clear_cache();
        }
        if controls::button(ui, "Reload reviews", controls::ButtonRole::Secondary)
            .hover_hint("Read the reviews folder again, after its files change.")
            .clicked()
        {
            state.reload = true;
        }
        let label = if state.show_credits {
            "Hide credits"
        } else {
            "Credits and rights"
        };
        if controls::button(ui, label, controls::ButtonRole::Secondary).clicked() {
            state.show_credits = !state.show_credits;
        }
    });
    if let Some(message) = &state.cache_message {
        ui.label(message);
    }
    if state.show_credits {
        credits(ui);
    }
}

/// The sources of the reviews and the rights in their content.
fn credits(ui: &mut egui::Ui) {
    let section = |ui: &mut egui::Ui, heading: &str, text: &str, links: &[(&str, &str, &str)]| {
        ui.label(egui::RichText::new(heading).color(roles().legend).strong());
        ui.label(text);
        for (source, use_, url) in links {
            ui.horizontal_wrapped(|ui| {
                ui.hyperlink_to(*source, *url);
                ui.label(*use_);
            });
        }
        ui.add_space(4.0);
    };
    section(
        ui,
        "Musical sources",
        "Track lists, durations, release data, and production context of the analyses.",
        &[
            (
                "MusicBrainz",
                "Track lists, durations, and release metadata.",
                "https://musicbrainz.org/",
            ),
            (
                "Wikipedia",
                "Context, production notes, release history, and genres.",
                "https://en.wikipedia.org/",
            ),
            (
                "YouTube",
                "Listening links on album pages.",
                "https://www.youtube.com/",
            ),
            (
                "Bandcamp",
                "Listening links on album pages.",
                "https://bandcamp.com/",
            ),
        ],
    );
    section(
        ui,
        "Cover art",
        "Covers are shown under fair use to identify each album. All rights stay with \
         their copyright holders. Select Display cover art to download a cover. The \
         application keeps it only in its cover cache. Each album page \
         names the cover's source.",
        &[
            (
                "Wikimedia Commons and Wikipedia",
                "The source of most cover links.",
                "https://commons.wikimedia.org/",
            ),
            (
                "Deezer",
                "The cover link of Quake.",
                "https://www.deezer.com/",
            ),
            (
                "Bandcamp",
                "The cover link of The Ocean Palace.",
                "https://bandcamp.com/",
            ),
        ],
    );
    section(
        ui,
        "Research sources",
        "Lists and polls behind the research shortlist of future reviews: Pitchfork, \
         SensCritique, Optimistic Underground, Hyperreal, Scaruffi, Album of the Year and \
         AllMusic, Deep Cuts, BBC Radio 3, RateYourMusic, Psy-Amb, the r/ambientmusic poll, \
         and AcclaimedMusic.",
        &[],
    );
    section(
        ui,
        "Analyses",
        "The analyses are deep-listening notes written for this collection. Timestamps are \
         approximate.",
        &[],
    );
}
