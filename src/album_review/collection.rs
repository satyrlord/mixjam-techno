//! The collection rules: search, artist and genre filters, the genre tag
//! row, and the sort orders of the cover grid.
use super::Review;
use std::{
    cmp::Ordering,
    collections::{BTreeMap, BTreeSet},
};

/// How many genre tags the tag row shows before "+N more".
pub const TOP_GENRES: usize = 6;

/// Lowercase text without accents, for search: "Oxygène" folds to "oxygene".
pub fn fold(text: &str) -> String {
    deunicode::deunicode(text).to_lowercase()
}

/// A comparison key that joins spelling variants: "New Age" and "New-Age".
pub fn fold_key(text: &str) -> String {
    fold(text)
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .collect()
}

/// The order of the cover grid. Every order breaks ties by year, then
/// artist, then title.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub enum Sort {
    #[default]
    YearAscending,
    YearDescending,
    Artist,
    Title,
    MostTracks,
}

impl Sort {
    pub const ALL: [Self; 5] = [
        Self::YearAscending,
        Self::YearDescending,
        Self::Artist,
        Self::Title,
        Self::MostTracks,
    ];

    pub fn label(self) -> &'static str {
        match self {
            Self::YearAscending => "Year ↑",
            Self::YearDescending => "Year ↓",
            Self::Artist => "Artist A–Z",
            Self::Title => "Title A–Z",
            Self::MostTracks => "Most tracks",
        }
    }

    /// The name that agent requests use.
    pub fn key(self) -> &'static str {
        match self {
            Self::YearAscending => "year",
            Self::YearDescending => "year_desc",
            Self::Artist => "artist",
            Self::Title => "title",
            Self::MostTracks => "tracks",
        }
    }

    pub fn parse(text: &str) -> Result<Self, String> {
        Self::ALL
            .into_iter()
            .find(|sort| sort.key().eq_ignore_ascii_case(text.trim()))
            .ok_or_else(|| {
                format!("The sort \"{text}\" is not year, year_desc, artist, title, or tracks.")
            })
    }

    pub fn compare(self, a: &Review, b: &Review) -> Ordering {
        let text = |a: &str, b: &str| fold(a).cmp(&fold(b)).then_with(|| a.cmp(b));
        let primary = match self {
            Self::YearAscending => Ordering::Equal,
            Self::YearDescending => b.year.cmp(&a.year),
            Self::Artist => text(&a.artist, &b.artist),
            Self::Title => text(&a.title, &b.title),
            Self::MostTracks => b.tracks.len().cmp(&a.tracks.len()),
        };
        primary
            .then_with(|| a.year.cmp(&b.year))
            .then_with(|| text(&a.artist, &b.artist))
            .then_with(|| text(&a.title, &b.title))
    }
}

/// The search text and the selected filters of the cover grid.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Filter {
    pub search: String,
    /// Selected artists. A review by any of them matches (union).
    pub artists: BTreeSet<String>,
    /// Selected genre tags. A review must have all of them (intersection).
    pub genres: BTreeSet<String>,
}

impl Filter {
    pub fn is_empty(&self) -> bool {
        self.search.trim().is_empty() && self.artists.is_empty() && self.genres.is_empty()
    }

    pub fn clear(&mut self) {
        *self = Self::default();
    }

    /// Whether a review matches the search and every filter. The search
    /// ignores case and accents and looks in the title, the artist, and the
    /// genre tags.
    pub fn matches(&self, review: &Review) -> bool {
        let artist = self.artists.is_empty() || self.artists.contains(&review.artist);
        let keys: BTreeSet<String> = review.genre_tags.iter().map(|tag| fold_key(tag)).collect();
        let genres = self.genres.iter().all(|tag| keys.contains(&fold_key(tag)));
        let query = fold(self.search.trim());
        let search = query.is_empty()
            || fold(&review.title).contains(&query)
            || fold(&review.artist).contains(&query)
            || review
                .genre_tags
                .iter()
                .any(|tag| fold(tag).contains(&query));
        artist && genres && search
    }

    /// Whether adding the genre tag to the selection still leaves a match.
    pub fn genre_has_results(&self, reviews: &[Review], tag: &str) -> bool {
        let mut candidate = self.clone();
        candidate.genres.insert(tag.to_owned());
        reviews.iter().any(|review| candidate.matches(review))
    }
}

/// The indices of the reviews that match, in the chosen order.
pub fn visible(reviews: &[Review], filter: &Filter, sort: Sort) -> Vec<usize> {
    let mut found: Vec<usize> = (0..reviews.len())
        .filter(|&index| filter.matches(&reviews[index]))
        .collect();
    found.sort_by(|&a, &b| sort.compare(&reviews[a], &reviews[b]));
    found
}

/// Every artist once, in A–Z order.
pub fn artists(reviews: &[Review]) -> Vec<String> {
    let mut artists: Vec<String> = reviews
        .iter()
        .map(|review| review.artist.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect();
    artists.sort_by(|a, b| fold(a).cmp(&fold(b)).then_with(|| a.cmp(b)));
    artists
}

/// Every genre tag with the number of reviews that have it, most used
/// first, then A–Z. Spelling variants count as one tag, shown in the
/// variant that most reviews use.
pub fn genre_counts(reviews: &[Review]) -> Vec<(String, usize)> {
    let mut variants: BTreeMap<String, BTreeMap<&str, usize>> = BTreeMap::new();
    let mut reviews_per_key: BTreeMap<String, usize> = BTreeMap::new();
    for review in reviews {
        let mut seen = BTreeSet::new();
        for tag in &review.genre_tags {
            let key = fold_key(tag);
            *variants
                .entry(key.clone())
                .or_default()
                .entry(tag.as_str())
                .or_default() += 1;
            if seen.insert(key.clone()) {
                *reviews_per_key.entry(key).or_default() += 1;
            }
        }
    }
    let mut counts: Vec<(String, usize)> = variants
        .into_iter()
        .map(|(key, spellings)| {
            let name = spellings
                .iter()
                .max_by(|a, b| a.1.cmp(b.1).then_with(|| b.0.cmp(a.0)))
                .map(|(name, _)| (*name).to_owned())
                .unwrap_or_default();
            (name, reviews_per_key[&key])
        })
        .collect();
    counts.sort_by(|a, b| {
        b.1.cmp(&a.1)
            .then_with(|| fold(&a.0).cmp(&fold(&b.0)))
            .then_with(|| a.0.cmp(&b.0))
    });
    counts
}

/// The result count: "visible / total albums", or "album" for a total of one.
pub fn count_label(visible: usize, total: usize) -> String {
    format!(
        "{visible} / {total} {}",
        if total == 1 { "album" } else { "albums" }
    )
}
