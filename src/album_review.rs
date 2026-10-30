//! Album Review: deep-listening analyses of albums, one JSON file per review
//! in the runtime folder `assets/album-review`. The application reads the
//! folder at run time and writes validated reviews back into it.
pub mod collection;
pub mod covers;
mod review;

pub use review::{
    Energy, Event, Kind, ROLE_PREFIX, Review, Track, clock_seconds, parse, runtime_seconds,
    valid_id, web_host,
};
use serde::Serialize;
use std::path::{Path, PathBuf};

/// The name of the reviews folder under the application's `assets`.
pub const FOLDER: &str = "album-review";

/// The reviews of one folder, sorted by id, and the files that did not load.
#[derive(Debug, Default)]
pub struct Shelf {
    folder: Option<PathBuf>,
    reviews: Vec<Review>,
    problems: Vec<(String, String)>,
    revision: u64,
}

/// A short form of a review, for lists.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Summary {
    pub id: String,
    pub kind: Kind,
    pub artist: String,
    pub title: String,
    pub year: i32,
    pub genre_tags: Vec<String>,
    pub runtime: String,
    pub tracks: usize,
}

impl From<&Review> for Summary {
    fn from(review: &Review) -> Self {
        Self {
            id: review.id.clone(),
            kind: review.kind,
            artist: review.artist.clone(),
            title: review.title.clone(),
            year: review.year,
            genre_tags: review.genre_tags.clone(),
            runtime: review.runtime.clone(),
            tracks: review.tracks.len(),
        }
    }
}

/// One track and the review it belongs to, for track queries.
#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TrackMatch {
    pub review: String,
    pub artist: String,
    pub album: String,
    pub year: i32,
    pub kind: Kind,
    pub track: Track,
}

/// What agents can ask of the reviews. A filter that is `None` matches all.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Query {
    /// A review id.
    pub id: Option<String>,
    /// Part of the artist's name, without regard to case or accents.
    pub artist: Option<String>,
    /// A track tag or a genre tag of the album, without regard to case,
    /// accents, spaces, or hyphens.
    pub tag: Option<String>,
    pub energy: Option<Energy>,
    pub kind: Option<Kind>,
    /// Search text, as in the window: title, artist, and genre tags.
    pub search: Option<String>,
}

impl Query {
    fn review_matches(&self, review: &Review) -> bool {
        self.id.as_ref().is_none_or(|id| &review.id == id)
            && self.kind.is_none_or(|kind| review.kind == kind)
            && self.artist.as_ref().is_none_or(|artist| {
                collection::fold(&review.artist).contains(&collection::fold(artist.trim()))
            })
            && self.search.as_ref().is_none_or(|search| {
                collection::Filter {
                    search: search.clone(),
                    ..Default::default()
                }
                .matches(review)
            })
    }

    fn genre_matches(&self, review: &Review) -> bool {
        self.tag.as_ref().is_some_and(|tag| {
            let key = collection::fold_key(tag);
            review
                .genre_tags
                .iter()
                .any(|genre| collection::fold_key(genre) == key)
        })
    }

    fn track_matches(&self, review: &Review, track: &Track) -> bool {
        let tag = self.tag.is_none()
            || self.genre_matches(review)
            || self.tag.as_ref().is_some_and(|tag| {
                let key = collection::fold_key(tag);
                track
                    .tags
                    .iter()
                    .any(|own| collection::fold_key(own) == key)
            });
        tag && self.energy.is_none_or(|energy| track.energy == energy)
    }

    /// Whether a review matches, with at least one track that matches the
    /// track filters.
    pub fn matches(&self, review: &Review) -> bool {
        self.review_matches(review)
            && review
                .tracks
                .iter()
                .any(|track| self.track_matches(review, track))
    }
}

impl Shelf {
    /// The reviews of the application's own reviews folder.
    pub fn open_default() -> Self {
        Self::open(crate::runtime_data::find(FOLDER))
    }

    /// The reviews of `folder`. `None` means that there is no reviews folder.
    pub fn open(folder: Option<PathBuf>) -> Self {
        let mut shelf = Self {
            folder,
            ..Default::default()
        };
        shelf.reload();
        shelf
    }

    /// Read the folder again.
    pub fn reload(&mut self) {
        self.reviews.clear();
        self.problems.clear();
        self.revision += 1;
        let Some(folder) = &self.folder else {
            return;
        };
        let listing = match std::fs::read_dir(folder) {
            Ok(listing) => listing,
            Err(error) => {
                self.problems
                    .push((display(folder), format!("Cannot read the folder: {error}")));
                return;
            }
        };
        let mut files: Vec<PathBuf> = listing
            .flatten()
            .map(|entry| entry.path())
            .filter(|path| {
                path.extension()
                    .is_some_and(|extension| extension == "json")
                    && path.is_file()
            })
            .collect();
        files.sort();
        for path in files {
            let name = path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default();
            let loaded = std::fs::read_to_string(&path)
                .map_err(|error| error.to_string())
                .and_then(|text| parse(&text))
                .and_then(|review| {
                    let stem = path.file_stem().and_then(|stem| stem.to_str());
                    if stem == Some(review.id.as_str()) {
                        Ok(review)
                    } else {
                        Err(format!(
                            "The id \"{}\" does not match the file name.",
                            review.id
                        ))
                    }
                });
            match loaded {
                Ok(review) => self.reviews.push(review),
                Err(error) => self.problems.push((name, error)),
            }
        }
        self.reviews.sort_by(|a, b| a.id.cmp(&b.id));
    }

    /// The reviews folder, or `None` when the application has none.
    pub fn folder(&self) -> Option<&Path> {
        self.folder.as_deref()
    }

    pub fn reviews(&self) -> &[Review] {
        &self.reviews
    }

    /// Files that did not load, each with the reason.
    pub fn problems(&self) -> &[(String, String)] {
        &self.problems
    }

    /// Advances each time the reviews change.
    pub fn revision(&self) -> u64 {
        self.revision
    }

    pub fn review(&self, id: &str) -> Option<&Review> {
        self.reviews
            .binary_search_by(|review| review.id.as_str().cmp(id))
            .ok()
            .map(|index| &self.reviews[index])
    }

    /// The reviews that match, in id order.
    pub fn find(&self, query: &Query) -> Vec<&Review> {
        self.reviews
            .iter()
            .filter(|review| query.matches(review))
            .collect()
    }

    /// The tracks that match, in review order and track order.
    pub fn tracks(&self, query: &Query) -> Vec<TrackMatch> {
        self.reviews
            .iter()
            .filter(|review| query.review_matches(review))
            .flat_map(|review| {
                review
                    .tracks
                    .iter()
                    .filter(|track| query.track_matches(review, track))
                    .map(|track| TrackMatch {
                        review: review.id.clone(),
                        artist: review.artist.clone(),
                        album: review.title.clone(),
                        year: review.year,
                        kind: review.kind,
                        track: track.clone(),
                    })
            })
            .collect()
    }

    /// Save a new review. It must obey every rule and have a new id.
    pub fn add(&mut self, review: Review) -> Result<(), String> {
        if self.review(&review.id).is_some() {
            return Err(format!(
                "A review with the id \"{}\" exists. Use update_review to change it.",
                review.id
            ));
        }
        self.save(review, true)
    }

    /// Replace a review that exists. The new one must obey every rule.
    pub fn update(&mut self, review: Review) -> Result<(), String> {
        if self.review(&review.id).is_none() {
            return Err(format!(
                "No review has the id \"{}\". Use add_review to add it.",
                review.id
            ));
        }
        self.save(review, false)
    }

    fn save(&mut self, review: Review, new: bool) -> Result<(), String> {
        let folder = self
            .folder
            .clone()
            .ok_or("There is no reviews folder, so the review cannot be saved.")?;
        let issues = review.issues();
        if !issues.is_empty() {
            return Err(format!(
                "The review breaks {} rule{}: {}",
                issues.len(),
                if issues.len() == 1 { "" } else { "s" },
                issues.join("; ")
            ));
        }
        let path = folder.join(format!("{}.json", review.id));
        let text = review.to_file_text();
        if new {
            crate::atomic_file::write_new(&path, text.as_bytes())?;
        } else {
            crate::atomic_file::write(&path, text.as_bytes())?;
        }
        match self
            .reviews
            .binary_search_by(|other| other.id.cmp(&review.id))
        {
            Ok(index) => self.reviews[index] = review,
            Err(index) => self.reviews.insert(index, review),
        }
        self.revision += 1;
        Ok(())
    }
}

/// A review from JSON that an agent sent. Saving checks its rules.
pub fn from_value(value: serde_json::Value) -> Result<Review, String> {
    serde_json::from_value(value)
        .map_err(|error| format!("The review has the wrong shape: {error}"))
}

/// A review from a JSON file. Saving checks its rules.
pub fn read_file(path: &Path) -> Result<Review, String> {
    let text = std::fs::read_to_string(path)
        .map_err(|error| format!("Cannot read {}: {error}", path.display()))?;
    serde_json::from_str(&text).map_err(|error| format!("The review has the wrong shape: {error}"))
}

fn display(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

#[cfg(test)]
mod tests;
