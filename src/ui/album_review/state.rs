//! The state of the Album Review window: the reviews, the filter and
//! sort, the covers, and the grid's derived lists.
use super::*;
use mixjam_techno::application::album_review::{
    Review, Shelf,
    collection::{self, Filter, Sort},
    covers::{CoverCache, CoverLoader, CoverOutcome},
};
use std::collections::BTreeMap;

/// What the window shows and the covers it has loaded. The reviews load
/// when the window or an agent first needs them.
#[derive(Default)]
pub(in super::super) struct AlbumReviewState {
    pub(super) shelf: Option<Shelf>,
    pub(in super::super) filter: Filter,
    pub(in super::super) sort: Sort,
    pub(super) genres_expanded: bool,
    /// The review whose page shows; `None` shows the grid.
    pub(in super::super) page: Option<String>,
    /// The view last drawn in the shared window's scroll area.
    pub(super) drawn_page: Option<String>,
    pub(super) covers: Option<CoverLoader>,
    pub(super) textures: BTreeMap<String, egui::TextureHandle>,
    /// Reviews whose cache was already looked at for the grid.
    pub(super) checked: BTreeSet<String>,
    /// Why a cover the user asked for is not shown.
    pub(super) cover_errors: BTreeMap<String, String>,
    pub(super) cache_usage: Option<(usize, u64)>,
    pub(super) cache_message: Option<String>,
    pub(super) show_credits: bool,
    /// Reload reviews was selected; the grid reads the folder again.
    pub(super) reload: bool,
    /// The track whose timeline row was selected; the page scrolls to it.
    pub(super) reveal_track: Option<u32>,
    /// A cover cache other than the data folder's, for tests.
    pub(super) cache_override: Option<CoverCache>,
    pub(super) grid: GridCache,
    /// The review of the open page, copied from the shelf once for each
    /// revision of the shelf, with the revision.
    pub(super) page_review: Option<(String, u64, Rc<Review>)>,
}

/// The grid's derived lists. Filtering folds every title, artist, and tag,
/// and each genre chip asks whether it still finds an album, so they are
/// computed again only when the reviews, the filter, or the sort change.
#[derive(Default)]
pub(super) struct GridCache {
    pub(super) revision: Option<u64>,
    pub(super) artists: Vec<String>,
    pub(super) genres: Vec<(String, usize)>,
    pub(super) filtered: Option<(Filter, Sort)>,
    pub(super) visible: Vec<usize>,
    /// For each of `genres`: whether adding it to the filter leaves a match.
    pub(super) has_results: Vec<bool>,
}

impl GridCache {
    pub(super) fn update(
        &mut self,
        revision: u64,
        reviews: &[Review],
        filter: &Filter,
        sort: Sort,
    ) {
        if self.revision != Some(revision) {
            self.revision = Some(revision);
            self.artists = collection::artists(reviews);
            self.genres = collection::genre_counts(reviews);
            self.filtered = None;
        }
        if self
            .filtered
            .as_ref()
            .is_some_and(|(cached, cached_sort)| cached == filter && *cached_sort == sort)
        {
            return;
        }
        self.visible = collection::visible(reviews, filter, sort);
        self.has_results = self
            .genres
            .iter()
            .map(|(genre, _)| filter.genre_has_results(reviews, genre))
            .collect();
        self.filtered = Some((filter.clone(), sort));
    }
}

impl AlbumReviewState {
    /// The reviews, read from the application's reviews folder on first use.
    pub(in super::super) fn shelf(&mut self) -> &mut Shelf {
        self.shelf.get_or_insert_with(Shelf::open_default)
    }

    /// Show the reviews of another folder, or of none.
    pub(in super::super) fn use_folder(&mut self, folder: Option<PathBuf>, cache: CoverCache) {
        self.shelf = Some(Shelf::open(folder));
        // A new shelf counts its revisions from the start again.
        self.grid = GridCache::default();
        self.page_review = None;
        self.cache_override = Some(cache);
        self.covers = None;
        self.textures.clear();
        self.checked.clear();
    }

    /// The review `id`, shared with the frames that draw its page.
    pub(super) fn page_review(&mut self, id: &str) -> Option<Rc<Review>> {
        let revision = self.shelf().revision();
        if let Some((cached, at, review)) = &self.page_review
            && cached == id
            && *at == revision
        {
            return Some(Rc::clone(review));
        }
        let review = Rc::new(self.shelf().review(id)?.clone());
        self.page_review = Some((id.to_owned(), revision, Rc::clone(&review)));
        Some(review)
    }

    pub(super) fn covers(&mut self) -> &mut CoverLoader {
        let cache = self.cache_override.clone();
        self.covers.get_or_insert_with(|| {
            CoverLoader::new(cache.unwrap_or_else(CoverCache::in_data_folder))
        })
    }

    /// Take the covers that the background thread finished.
    pub(super) fn poll(&mut self, ctx: &egui::Context) {
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

    pub(super) fn cache_usage(&mut self) -> (usize, u64) {
        if self.cache_usage.is_none() {
            let usage = self.covers().cache().usage();
            self.cache_usage = Some(usage);
        }
        self.cache_usage.unwrap_or_default()
    }

    pub(super) fn clear_cache(&mut self) {
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
pub(in super::super) fn capture_state(view: &str) -> AlbumReviewState {
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
