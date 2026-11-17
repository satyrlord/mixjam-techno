//! Queries over one painted frame, so a test names what it looks for instead
//! of walking the frame's shapes:
//!
//! - `screen(&output).text("Muted").color()`
//! - `screen(&output).within(row).shows("Solo elsewhere")`
//! - `screen(&output).node("Track 1 mute").toggled()`
//!
//! A query that must find something panics with what it looked for.

use super::*;
use egui::accesskit::{self, Role, Toggled};
use egui::epaint::TextShape;

/// One frame's painted texts and accessible nodes, optionally only those in
/// an area.
#[derive(Clone, Copy)]
pub(crate) struct Screen<'a> {
    output: &'a egui::FullOutput,
    area: Option<Rect>,
}

pub(crate) fn screen(output: &egui::FullOutput) -> Screen<'_> {
    Screen { output, area: None }
}

impl<'a> Screen<'a> {
    /// Only what lies in `area`: a text or node whose centre is in it.
    pub(crate) fn within(self, area: Rect) -> Self {
        Self {
            area: Some(self.area.map_or(area, |outer| outer.intersect(area))),
            ..self
        }
    }

    fn holds(self, rect: Rect) -> bool {
        self.area.is_none_or(|area| area.contains(rect.center()))
    }

    /// Every painted text, in paint order.
    pub(crate) fn texts(self) -> impl Iterator<Item = Text<'a>> {
        self.output
            .shapes
            .iter()
            .filter_map(|shape| match &shape.shape {
                egui::Shape::Text(text) => Some(Text {
                    shape: text,
                    clip: shape.clip_rect,
                }),
                _ => None,
            })
            .filter(move |text| self.holds(text.rect()))
    }

    /// The first text painted as exactly `label`.
    pub(crate) fn find(self, label: &str) -> Option<Text<'a>> {
        self.texts().find(|text| text.label() == label)
    }

    /// The first text painted as exactly `label`; panics if there is none.
    pub(crate) fn text(self, label: &str) -> Text<'a> {
        self.find(label)
            .unwrap_or_else(|| panic!("Missing text: {label}"))
    }

    /// Whether a text painted as exactly `label` shows.
    pub(crate) fn shows(self, label: &str) -> bool {
        self.find(label).is_some()
    }

    /// Every accessible node of the frame. Accessibility must be enabled.
    pub(crate) fn nodes(self) -> impl Iterator<Item = Node<'a>> {
        self.output
            .platform_output
            .accesskit_update
            .as_ref()
            .expect("Accessibility must be enabled for this test.")
            .nodes
            .iter()
            .map(|(id, node)| Node { id: *id, node })
            .filter(move |node| node.bounds().is_none_or(|rect| self.holds(rect)))
    }

    /// The accessible node named `name`, if any.
    pub(crate) fn find_node(self, name: &str) -> Option<Node<'a>> {
        self.nodes().find(|node| node.node.label() == Some(name))
    }

    /// The accessible node named `name`; panics if there is none.
    pub(crate) fn node(self, name: &str) -> Node<'a> {
        self.find_node(name)
            .unwrap_or_else(|| panic!("Missing accessible control: {name}"))
    }

    /// How many accessible nodes are named `name`.
    pub(crate) fn named_count(self, name: &str) -> usize {
        self.nodes()
            .filter(|node| node.node.label() == Some(name))
            .count()
    }

    /// A painted label, which egui names by its value.
    pub(crate) fn label(self, value: &str) -> Node<'a> {
        self.nodes()
            .find(|node| node.node.role() == Role::Label && node.node.value() == Some(value))
            .unwrap_or_else(|| panic!("Missing accessible label: {value}"))
    }
}

/// A painted text and the clip it was painted under.
#[derive(Clone, Copy)]
pub(crate) struct Text<'a> {
    shape: &'a TextShape,
    clip: Rect,
}

impl<'a> Text<'a> {
    pub(crate) fn label(self) -> &'a str {
        &self.shape.galley.job.text
    }

    /// Its position and the galley's size.
    pub(crate) fn rect(self) -> Rect {
        Rect::from_min_size(self.shape.pos, self.shape.galley.size())
    }

    /// The galley's own bounds at its position, which can start left of
    /// `rect` for a centred or right-aligned layout.
    pub(crate) fn bounds(self) -> Rect {
        self.shape.galley.rect.translate(self.shape.pos.to_vec2())
    }

    /// The clip rectangle it was painted under.
    pub(crate) fn clip(self) -> Rect {
        self.clip
    }

    /// The part of its bounds that shows inside its clip.
    pub(crate) fn visible(self) -> Rect {
        self.bounds().intersect(self.clip)
    }

    /// Whether its clip shows all of it.
    pub(crate) fn whole(self) -> bool {
        self.clip.contains_rect(self.rect())
    }

    /// The colour its glyphs are drawn in: the override, else the first
    /// section's colour, with a placeholder resolved to the fallback.
    pub(crate) fn color(self) -> Color32 {
        drawn_text_color(self.shape)
    }

    /// The colour its first glyph reaches the screen in, with any opacity the
    /// painter baked into the glyphs. `color` reports the colour asked for.
    pub(crate) fn glyph_color(self) -> Color32 {
        let vertex = self
            .shape
            .galley
            .rows
            .iter()
            .find_map(|row| row.row.visuals.mesh.vertices.first())
            .map(|vertex| vertex.color);
        let color = match (self.shape.override_text_color, vertex) {
            (Some(color), _) => color,
            (None, Some(color)) if color != Color32::PLACEHOLDER => color,
            _ => self.shape.fallback_color,
        };
        color.gamma_multiply(self.shape.opacity_factor)
    }

    /// The opacity it was painted at, such as a dimmed row's: its glyphs'
    /// alpha against the alpha of the colour asked for.
    pub(crate) fn opacity(self) -> f32 {
        f32::from(self.glyph_color().a()) / f32::from(self.color().a().max(1))
    }

    /// The first section's format: its font, colour, and decorations.
    pub(crate) fn format(self) -> &'a egui::TextFormat {
        &self.shape.galley.job.sections[0].format
    }

    /// How many rows the galley laid out.
    pub(crate) fn rows(self) -> usize {
        self.shape.galley.rows.len()
    }

    /// The text shape itself, for a check this module does not name.
    pub(crate) fn shape(self) -> &'a TextShape {
        self.shape
    }
}

/// An accessible node of the frame.
#[derive(Clone, Copy)]
pub(crate) struct Node<'a> {
    id: accesskit::NodeId,
    node: &'a accesskit::Node,
}

impl<'a> Node<'a> {
    pub(crate) fn id(self) -> accesskit::NodeId {
        self.id
    }

    /// The accesskit node itself, for a property this module does not name.
    pub(crate) fn node(self) -> &'a accesskit::Node {
        self.node
    }

    /// Its bounds on screen, if it has any.
    pub(crate) fn bounds(self) -> Option<Rect> {
        self.node.bounds().map(|bounds| {
            Rect::from_min_max(
                Pos2::new(bounds.x0 as f32, bounds.y0 as f32),
                Pos2::new(bounds.x1 as f32, bounds.y1 as f32),
            )
        })
    }

    /// Its bounds on screen; panics if it has none.
    pub(crate) fn rect(self) -> Rect {
        self.bounds()
            .unwrap_or_else(|| panic!("Accessible control {:?} has no bounds.", self.node.label()))
    }

    /// Whether it is a checked switch.
    pub(crate) fn toggled(self) -> bool {
        self.node.toggled() == Some(Toggled::True)
    }

    pub(crate) fn role(self) -> Role {
        self.node.role()
    }

    pub(crate) fn value(self) -> Option<&'a str> {
        self.node.value()
    }
}
