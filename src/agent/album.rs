//! Answers to Album Review requests from the reviews of a shelf.
use super::{Body, Request, ReviewProblem};
use crate::album_review;
use std::path::{Path, PathBuf};

/// Answer an Album Review request from the reviews of `shelf`, which
/// `add_review` and `update_review` change. Any other request gives `None`.
pub fn album_reply(
    shelf: &mut album_review::Shelf,
    request: &Request,
) -> Option<Result<Body, String>> {
    use album_review::{Energy, Kind, Query, collection};
    fn parse<T>(
        text: &Option<String>,
        f: fn(&str) -> Result<T, String>,
    ) -> Result<Option<T>, String> {
        text.as_deref().map(f).transpose()
    }
    let query = |id: &Option<String>,
                 artist: &Option<String>,
                 tag: &Option<String>,
                 energy: &Option<String>,
                 kind: &Option<String>,
                 search: &Option<String>|
     -> Result<Query, String> {
        Ok(Query {
            id: id.clone(),
            artist: artist.clone(),
            tag: tag.clone(),
            energy: parse(energy, Energy::parse)?,
            kind: parse(kind, Kind::parse)?,
            search: search.clone(),
        })
    };
    let saved = |shelf: &mut album_review::Shelf,
                 review: &Option<serde_json::Value>,
                 file: &Option<PathBuf>,
                 added: bool|
     -> Result<Body, String> {
        let review = match (review, file) {
            (Some(value), None) => album_review::from_value(value.clone())?,
            (None, Some(path)) => album_review::read_file(path)?,
            _ => return Err("Give the review as review={...} or as file=PATH, not both.".into()),
        };
        let id = review.id.clone();
        if added {
            shelf.add(review)?;
        } else {
            shelf.update(review)?;
        }
        let path = shelf
            .folder()
            .map(|folder| folder.join(format!("{id}.json")))
            .unwrap_or_default();
        Ok(Body::ReviewSaved { id, path, added })
    };
    Some(match request {
        Request::Reviews {
            search,
            artist,
            tag,
            energy,
            kind,
            sort,
            offset,
            limit,
        } => query(&None, artist, tag, energy, kind, search).and_then(|query| {
            let sort = parse(sort, collection::Sort::parse)?.unwrap_or_default();
            let mut found = shelf.find(&query);
            found.sort_by(|a, b| sort.compare(a, b));
            Ok(Body::Reviews {
                folder: shelf.folder().map(Path::to_path_buf),
                total: found.len(),
                reviews: found
                    .into_iter()
                    .skip(*offset)
                    .take(limit.unwrap_or(50))
                    .map(album_review::Summary::from)
                    .collect(),
                problems: shelf
                    .problems()
                    .iter()
                    .map(|(file, error)| ReviewProblem {
                        file: file.clone(),
                        error: error.clone(),
                    })
                    .collect(),
            })
        }),
        Request::Review { id } => shelf
            .review(id)
            .map(|review| Body::Review {
                review: Box::new(review.clone()),
            })
            .ok_or_else(|| {
                if shelf.folder().is_none() {
                    "There is no reviews folder.".to_string()
                } else {
                    format!("No review has the id \"{id}\". Use `reviews` to list them.")
                }
            }),
        Request::ReviewTracks {
            id,
            artist,
            tag,
            energy,
            kind,
            offset,
            limit,
        } => query(id, artist, tag, energy, kind, &None).map(|query| {
            let found = shelf.tracks(&query);
            Body::ReviewTracks {
                total: found.len(),
                tracks: found
                    .into_iter()
                    .skip(*offset)
                    .take(limit.unwrap_or(50))
                    .collect(),
            }
        }),
        Request::AddReview { review, file } => saved(shelf, review, file, true),
        Request::UpdateReview { review, file } => saved(shelf, review, file, false),
        _ => return None,
    })
}
