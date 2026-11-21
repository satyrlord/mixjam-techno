//! The Inspector's clip pages. One clip shows a picture of its length and
//! fades, whose handles drag, above cards of exact values: its fades, its
//! sound, and its place. Several clips share the fade and sound cards; a
//! change there sets every selected clip at once, as one undo step.
use super::*;

#[path = "cards.rs"]
mod cards;
#[path = "fields.rs"]
mod fields;
#[path = "pages.rs"]
mod pages;
#[path = "picture.rs"]
mod picture;

/// The height of one field row in a card.
const ROW: f32 = 28.0;
/// The height of the clip's picture.
const PICTURE: f32 = 56.0;
/// Where a card's value column starts, from the card's left edge.
const VALUES: f32 = 72.0;
/// The text that a field shows when the selected clips differ.
const MIXED: &str = "–";
