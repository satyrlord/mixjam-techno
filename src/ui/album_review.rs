//! Album Review window: the cover grid with its search, filters, and sort,
//! and the album page with the overview, links, and track timelines. The
//! reviews and the cover cache stay in the application layer.
use super::*;
use mixjam_techno::application::album_review::{
    Energy, Kind, Review, Shelf, Track, clock_seconds,
    collection::{self, Filter, Sort, TOP_GENRES},
    covers::{CoverCache, CoverLoader, CoverOutcome},
};
use std::collections::BTreeMap;

pub(super) const TITLE: &str = "Album Review";
const CARD: Vec2 = Vec2::new(150.0, 214.0);
const COVER: f32 = 138.0;
const PAGE_COVER: f32 = 220.0;

/// What the window shows and the covers it has loaded. The reviews load
/// when the window or an agent first needs them.
#[derive(Default)]
pub(super) struct AlbumReviewState {
    shelf: Option<Shelf>,
    pub(super) filter: Filter,
    pub(super) sort: Sort,
    genres_expanded: bool,
    /// The review whose page shows; `None` shows the grid.
    pub(super) page: Option<String>,
    /// The view last drawn in the shared window's scroll area.
    drawn_page: Option<String>,
    covers: Option<CoverLoader>,
    textures: BTreeMap<String, egui::TextureHandle>,
    /// Reviews whose cache was already looked at for the grid.
    checked: BTreeSet<String>,
    /// Why a cover the user asked for is not shown.
    cover_errors: BTreeMap<String, String>,
    cache_usage: Option<(usize, u64)>,
    cache_message: Option<String>,
    show_credits: bool,
    /// Reload reviews was selected; the grid reads the folder again.
    reload: bool,
    /// The track whose timeline row was selected; the page scrolls to it.
    reveal_track: Option<u32>,
    /// A cover cache other than the data folder's, for tests.
    cache_override: Option<CoverCache>,
}

impl AlbumReviewState {
    /// The reviews, read from the application's reviews folder on first use.
    pub(super) fn shelf(&mut self) -> &mut Shelf {
        self.shelf.get_or_insert_with(Shelf::open_default)
    }

    /// Show the reviews of another folder, or of none.
    pub(super) fn use_folder(&mut self, folder: Option<PathBuf>, cache: CoverCache) {
        self.shelf = Some(Shelf::open(folder));
        self.cache_override = Some(cache);
        self.covers = None;
        self.textures.clear();
        self.checked.clear();
    }

    fn covers(&mut self) -> &mut CoverLoader {
        let cache = self.cache_override.clone();
        self.covers.get_or_insert_with(|| {
            CoverLoader::new(cache.unwrap_or_else(CoverCache::in_data_folder))
        })
    }

    /// Take the covers that the background thread finished.
    fn poll(&mut self, ctx: &egui::Context) {
        let Some(loader) = self.covers.as_mut() else {
            return;
        };
        let results = loader.poll();
        let busy = loader.busy();
        if !results.is_empty() {
            self.cache_usage = None;
        }
        for result in results {
            match result.outcome {
                CoverOutcome::Ready(image) => {
                    let texture = ctx.load_texture(
                        format!("album-cover-{}", result.id),
                        egui::ColorImage::from_rgba_unmultiplied(
                            [image.width as usize, image.height as usize],
                            &image.rgba,
                        ),
                        egui::TextureOptions::LINEAR,
                    );
                    self.cover_errors.remove(&result.id);
                    self.textures.insert(result.id, texture);
                }
                CoverOutcome::NotCached => {}
                CoverOutcome::Failed(reason) => {
                    self.cover_errors.insert(result.id, reason);
                }
            }
        }
        if busy {
            ctx.request_repaint_after(Duration::from_millis(50));
        }
    }

    fn cache_usage(&mut self) -> (usize, u64) {
        if self.cache_usage.is_none() {
            let usage = self.covers().cache().usage();
            self.cache_usage = Some(usage);
        }
        self.cache_usage.unwrap_or_default()
    }

    fn clear_cache(&mut self) {
        // A worker may still write a downloaded file or return a decoded cover.
        // Its results must be consumed before the cache can be cleared.
        if self.covers().busy() {
            self.cache_message = Some("Wait for cover loading to finish.".into());
            return;
        }
        let result = self.covers().cache().clear();
        self.cache_message = Some(match result {
            Ok(0) => "The cover cache is empty.".into(),
            Ok(count) => format!(
                "Removed {count} cover{}.",
                if count == 1 { "" } else { "s" }
            ),
            Err(error) => error,
        });
        self.textures.clear();
        self.checked.clear();
        self.cover_errors.clear();
        self.cache_usage = None;
    }
}

/// The Album Review view of a capture: the grid, a filtered grid, an album
/// page, or a missing reviews folder. Captures run with their own data folder,
/// so the cover cache starts empty.
pub(super) fn capture_state(view: &str) -> AlbumReviewState {
    let mut state = AlbumReviewState::default();
    match view {
        "album-review-missing" => state.use_folder(None, CoverCache::in_data_folder()),
        "album-review-page" => state.page = Some("jean-michel-jarre-oxygene".into()),
        "album-review-filtered" => {
            state.filter.search = "eno".into();
            state.filter.genres.insert("Ambient".into());
        }
        _ => {}
    }
    state
}

pub(super) fn show(ctx: &egui::Context, open: &mut bool, state: &mut AlbumReviewState) {
    if !*open {
        return;
    }
    state.poll(ctx);
    let viewport = ctx.viewport_rect();
    dialog::window(TITLE)
        .open(open)
        .default_width((viewport.width() - 160.0).clamp(560.0, 980.0))
        .default_height((viewport.height() - 200.0).clamp(300.0, 660.0))
        .max_height((viewport.height() - 80.0).max(240.0))
        .constrain_to(viewport.shrink(12.0))
        .show(ctx, |ui| {
            ui.spacing_mut().item_spacing = Vec2::new(8.0, 8.0);
            if state.drawn_page != state.page {
                ui.scroll_to_cursor_animation(
                    Some(egui::Align::TOP),
                    egui::style::ScrollAnimation::none(),
                );
                state.drawn_page.clone_from(&state.page);
            }
            match state.page.clone() {
                Some(id) => page(ui, state, &id),
                None => grid(ui, state),
            }
        });
}

fn grid(ui: &mut egui::Ui, state: &mut AlbumReviewState) {
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
    let visible = collection::visible(reviews, &state.filter, state.sort);
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
    filter_rows(ui, state, reviews, !visible.is_empty());
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
        .layout_no_wrap(label.into(), FontId::proportional(13.0), controls::INK)
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
fn filter_rows(ui: &mut egui::Ui, state: &mut AlbumReviewState, reviews: &[Review], results: bool) {
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Artists")
                .color(controls::CREAM)
                .strong(),
        );
        for artist in collection::artists(reviews) {
            let active = state.filter.artists.contains(&artist);
            if chip(ui, &artist, active, &format!("Artist filter: {artist}")).clicked()
                && !state.filter.artists.remove(&artist)
            {
                state.filter.artists.insert(artist);
            }
        }
    });
    let genres = collection::genre_counts(reviews);
    let hidden = genres.len().saturating_sub(TOP_GENRES);
    ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new("Genres")
                .color(controls::CREAM)
                .strong(),
        );
        for (index, (genre, count)) in genres.iter().enumerate() {
            let active = state.filter.genres.contains(genre);
            if !state.genres_expanded && index >= TOP_GENRES && !active {
                continue;
            }
            let usable = active || state.filter.genre_has_results(reviews, genre);
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
                .on_disabled_hover_text("No album has this tag together with the selection.");
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
                .color(controls::CREAM)
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
    painter.rect_filled(rect, 4.0, controls::INK);
    let disc = rect.center() + Vec2::new(rect.width() * 0.12, 0.0);
    let radius = rect.width() * 0.3;
    painter.circle_stroke(
        disc,
        radius,
        Stroke::new(2.0_f32, MUTED.gamma_multiply(0.5)),
    );
    painter.circle_filled(disc, radius * 0.35, MUTED.gamma_multiply(0.35));
    let sleeve = Rect::from_center_size(
        rect.center() - Vec2::new(rect.width() * 0.12, 0.0),
        Vec2::splat(rect.width() * 0.56),
    );
    painter.rect_filled(sleeve, 3.0, controls::INK);
    painter.rect_stroke(
        sleeve,
        3.0,
        Stroke::new(2.0_f32, MUTED.gamma_multiply(0.6)),
        egui::StrokeKind::Inside,
    );
}

fn cover(ui: &egui::Ui, state: &AlbumReviewState, id: &str, rect: Rect) {
    match state.textures.get(id) {
        Some(texture) => {
            let size = texture.size_vec2();
            let scale = (rect.width() / size.x).min(rect.height() / size.y);
            let shown = Rect::from_center_size(rect.center(), size * scale);
            ui.painter().rect_filled(rect, 4.0, controls::INK);
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
            controls::FACE.gamma_multiply(1.4)
        } else {
            controls::FACE
        },
    );
    if response.has_focus() {
        ui.painter().rect_stroke(
            rect,
            5.0,
            Stroke::new(2.0_f32, controls::ACCENT),
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
    text(150.0, &review.artist.to_uppercase(), 11.0, MUTED);
    text(166.0, &review.title, 13.5, controls::CREAM);
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
    text(188.0, &detail, 11.5, MUTED);
    let name = format!("Open review: {} — {}", review.artist, review.title);
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Button, true, &name));
    response.on_hover_text(format!(
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
            ui.colored_label(CORAL, format!("{file}: {error}"));
        }
    });
}

fn footer(ui: &mut egui::Ui, state: &mut AlbumReviewState) {
    ui.add_space(6.0);
    ui.separator();
    let (count, bytes) = state.cache_usage();
    ui.horizontal_wrapped(|ui| {
        ui.label(format!(
            "Cover cache: {count} cover{}, {:.1} MB of 256 MB.",
            if count == 1 { "" } else { "s" },
            bytes as f64 / 1024.0 / 1024.0
        ));
        let busy = state.covers().busy();
        if ui
            .add_enabled_ui(!busy, |ui| {
                controls::button(ui, "Clear cover cache", controls::ButtonRole::Secondary)
            })
            .inner
            .on_hover_text("Remove every downloaded cover. Each cover downloads again on request.")
            .on_disabled_hover_text("Wait for cover loading to finish.")
            .clicked()
        {
            state.clear_cache();
        }
        if controls::button(ui, "Reload reviews", controls::ButtonRole::Secondary)
            .on_hover_text("Read the reviews folder again, after its files change.")
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
        ui.label(egui::RichText::new(heading).color(controls::CREAM).strong());
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
         their copyright holders. The application stores no cover: it downloads one when you \
         select Display cover art, and keeps it only in its cover cache. Each album page \
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

fn energy_color(energy: Energy) -> Color32 {
    match energy {
        Energy::Low => Color32::from_rgb(137, 176, 210),
        Energy::Mid => Color32::from_rgb(162, 196, 133),
        Energy::High => Color32::from_rgb(232, 197, 133),
        Energy::Peak => CORAL,
    }
}

/// A steady color for a section name, so that the same section reads the
/// same on every track.
fn section_color(section: &str) -> Color32 {
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

fn page(ui: &mut egui::Ui, state: &mut AlbumReviewState, id: &str) {
    let Some(review) = state.shelf().review(id).cloned() else {
        ui.label(format!(
            "The review \"{id}\" is no longer in the reviews folder."
        ));
        if controls::button(ui, "All albums", controls::ButtonRole::Primary).clicked() {
            state.page = None;
        }
        return;
    };
    let back = controls::button(ui, "← All albums", controls::ButtonRole::Secondary)
        .on_hover_text("Back to the cover grid");
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
                    .color(controls::CREAM),
            );
            ui.label(egui::RichText::new(&review.artist).size(15.0).color(MUTED));
            ui.add_space(4.0);
            egui::Grid::new("album-review-metadata")
                .num_columns(2)
                .spacing(Vec2::new(16.0, 4.0))
                .show(ui, |ui| {
                    let mut row = |name: &str, value: String| {
                        ui.add(
                            egui::Label::new(egui::RichText::new(name).color(MUTED))
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
            .color(controls::CREAM),
    );
    for paragraph in review.overview.split("\n\n") {
        ui.label(paragraph.trim());
    }
    ui.add_space(6.0);
    ui.label(
        egui::RichText::new("Timeline")
            .size(16.0)
            .color(controls::CREAM),
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
            .on_hover_text("Back to the top of the page")
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
                .on_hover_text(format!("Download the cover from {url}"))
                .on_disabled_hover_text("The cover is downloading.");
            if response.clicked() {
                state.cover_errors.remove(&review.id);
                state.covers().request(&review.id, Some(url));
            }
            if pending {
                ui.label("Downloading the cover…");
            } else if let Some(reason) = state.cover_errors.get(&review.id) {
                ui.colored_label(CORAL, format!("No cover: {reason}"));
            }
        }
        Some(_) => {}
    }
    if let Some(source) = &review.cover_source {
        ui.hyperlink_to("Cover source", source)
            .on_hover_text(source);
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
        ui.label(egui::RichText::new("No listening links.").color(MUTED));
        return;
    }
    ui.horizontal_wrapped(|ui| {
        for (label, url) in links {
            if let Some(url) = url {
                ui.hyperlink_to(label, url).on_hover_text(url);
            }
        }
    });
    for (label, path) in local {
        if let Some(path) = path {
            let shown = display_paths(path);
            ui.horizontal_wrapped(|ui| {
                ui.label(egui::RichText::new(label).color(MUTED));
                ui.add(egui::Label::new(&shown).truncate())
                    .on_hover_text(&shown);
            });
        }
    }
}

/// One row per track: each section from its timestamp to the next, as wide
/// as its share of the longest track. Returns the track of a clicked row.
fn timeline_chart(ui: &mut egui::Ui, review: &Review) -> Option<u32> {
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
            painter.layout_no_wrap(title.clone(), FontId::proportional(12.0), controls::CREAM);
        painter
            .with_clip_rect(Rect::from_min_size(
                row.min,
                Vec2::new(label_width - 8.0, 20.0),
            ))
            .galley(
                Pos2::new(row.left(), row.center().y - galley.size().y / 2.0),
                galley,
                controls::CREAM,
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
                Stroke::new(1.0_f32, controls::INK),
            );
        }
        painter.text(
            Pos2::new(x(duration) + 6.0, row.center().y),
            Align2::LEFT_CENTER,
            &track.duration,
            FontId::proportional(11.0),
            MUTED,
        );
        if response.hovered() {
            painter.rect_stroke(
                row.expand(1.0),
                2.0,
                Stroke::new(1.0_f32, controls::ACCENT),
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
            .on_hover_text(format!("{title}\n{}", sections.join("\n")))
            .clicked()
        {
            clicked = Some(track.num);
        }
    }
    clicked
}

fn track_section(ui: &mut egui::Ui, track: &Track, reveal: bool) {
    ui.add_space(4.0);
    let heading = ui.horizontal_wrapped(|ui| {
        ui.label(
            egui::RichText::new(format!("{}. {}", track.num, track.title))
                .size(15.0)
                .color(controls::CREAM),
        );
        ui.label(egui::RichText::new(&track.duration).color(MUTED));
        ui.label(
            egui::RichText::new(format!("Energy: {}", track.energy.key()))
                .color(energy_color(track.energy))
                .strong(),
        );
        ui.label(egui::RichText::new(track.tags.join(" · ")).color(MUTED));
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
                    ui.label(egui::RichText::new(detail).color(MUTED).size(12.5));
                }
            });
        });
    }
    ui.separator();
}

#[cfg(test)]
#[path = "album_review_state_tests.rs"]
mod tests;
