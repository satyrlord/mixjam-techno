//! The board's keys: the shared hardware key with the board's glyphs and
//! selected state, and Listen, which every pane shows.
use super::*;

/// What a board key shows before its text.
#[derive(Clone, Copy)]
pub(super) enum Glyph<'a> {
    None,
    Rail(&'a RailIcons, RailIcon),
    Play,
    Stop,
    /// A die's face, for Randomise.
    Dice,
}

impl<'a> Glyph<'a> {
    /// A rail icon when the icons loaded, else nothing.
    pub(super) fn rail(icons: Option<&'a RailIcons>, icon: RailIcon) -> Self {
        icons.map_or(Self::None, |icons| Self::Rail(icons, icon))
    }

    fn width(self, ctx: &egui::Context) -> f32 {
        match self {
            Self::None => 0.0,
            Self::Rail(icons, _) => icons.icon_rect(ctx, Pos2::ZERO).width(),
            Self::Play | Self::Stop => 10.0,
            Self::Dice => 13.0,
        }
    }

    fn paint(self, ui: &egui::Ui, painter: &egui::Painter, center: Pos2, color: Color32) {
        let snap = |point: Pos2| {
            egui::emath::GuiRounding::round_to_pixels(point, ui.ctx().pixels_per_point())
        };
        match self {
            Self::None => {}
            Self::Rail(icons, icon) => {
                painter.image(
                    icons.texture_id(icon),
                    icons.icon_rect(ui.ctx(), center),
                    Rect::from_min_max(Pos2::ZERO, Pos2::new(1.0, 1.0)),
                    color,
                );
            }
            Self::Play => {
                painter.add(egui::Shape::convex_polygon(
                    [
                        Vec2::new(-4.0, -5.0),
                        Vec2::new(-4.0, 5.0),
                        Vec2::new(5.0, 0.0),
                    ]
                    .map(|offset| snap(center + offset))
                    .to_vec(),
                    color,
                    Stroke::NONE,
                ));
            }
            Self::Stop => {
                painter.rect_filled(Rect::from_center_size(center, Vec2::splat(9.0)), 1.0, color);
            }
            Self::Dice => {
                painter.rect_stroke(
                    Rect::from_center_size(center, Vec2::splat(13.0)),
                    3.0,
                    Stroke::new(1.5_f32, color),
                    egui::StrokeKind::Inside,
                );
                for pip in [Vec2::new(-3.0, -3.0), Vec2::ZERO, Vec2::new(3.0, 3.0)] {
                    painter.circle_filled(center + pip, 1.3, color);
                }
            }
        }
    }
}

/// A board key. `name` is what assistive technology and agents read; a
/// choice key has `selected`, which lights it with the board's accent.
/// `id` keeps a key's focus when its name changes.
pub(super) struct Key<'a> {
    pub(super) id: &'a str,
    pub(super) name: &'a str,
    pub(super) text: &'a str,
    pub(super) glyph: Glyph<'a>,
    pub(super) enabled: bool,
    pub(super) primary: bool,
    pub(super) selected: Option<bool>,
}

impl<'a> Key<'a> {
    pub(super) fn new(name: &'a str, text: &'a str) -> Self {
        Self {
            id: name,
            name,
            text,
            glyph: Glyph::None,
            enabled: true,
            primary: false,
            selected: None,
        }
    }
}

pub(super) fn key(ui: &mut egui::Ui, rect: Rect, spec: Key<'_>) -> Response {
    let role = if spec.primary {
        controls::ButtonRole::Primary
    } else {
        controls::ButtonRole::Secondary
    };
    let response = controls::button_at(ui, rect, spec.id, "", role, spec.enabled);
    let down = response.is_pointer_button_down_on();
    // The face sits where the shared key paints it.
    let face = rect
        .shrink(2.0)
        .translate(Vec2::new(0.0, if down { 1.0 } else { -1.0 }));
    let painter = ui.painter().with_clip_rect(rect.intersect(ui.clip_rect()));
    let accent = controls::Palette::Olive.accent();
    if spec.selected == Some(true) {
        painter.rect_filled(face, 3.0, accent.gamma_multiply(0.22));
        painter.rect_stroke(
            face,
            3.0,
            Stroke::new(1.5_f32, accent),
            egui::StrokeKind::Inside,
        );
    }
    let color = if !response.enabled() {
        roles().disabled_text
    } else if spec.primary {
        roles().ink
    } else {
        roles().legend
    };
    let galley = (!spec.text.is_empty())
        .then(|| painter.layout_no_wrap(spec.text.to_owned(), FontId::proportional(13.0), color));
    let glyph_width = spec.glyph.width(ui.ctx());
    let text_width = galley.as_ref().map_or(0.0, |galley| galley.size().x);
    let gap = if glyph_width > 0.0 && text_width > 0.0 {
        6.0
    } else {
        0.0
    };
    let ppp = ui.ctx().pixels_per_point();
    let snap = |point: Pos2| egui::emath::GuiRounding::round_to_pixels(point, ppp);
    let left = face.center().x - (glyph_width + gap + text_width) / 2.0;
    let middle = face.center().y;
    let glyph_center = snap(Pos2::new(left + glyph_width / 2.0, middle));
    spec.glyph.paint(ui, &painter, glyph_center, color);
    if let Some(galley) = galley {
        let at = snap(Pos2::new(
            left + glyph_width + gap,
            middle - galley.size().y / 2.0,
        ));
        painter.galley(at, galley, color);
    }
    let (name, enabled) = (spec.name, response.enabled());
    response.widget_info(|| match spec.selected {
        Some(selected) => {
            egui::WidgetInfo::selected(egui::WidgetType::SelectableLabel, enabled, selected, name)
        }
        None => egui::WidgetInfo::labeled(egui::WidgetType::Button, enabled, name),
    });
    response
}

/// The hint of a key that needs a playable source.
pub(super) const NO_SOURCE: &str = "Choose an available source file or palette sound.";

/// Listen, which plays the pattern, or Stop while it plays. It leads the
/// grid's top row in both views. Narrow boards show its icon alone.
pub(super) fn listen_key(
    ui: &mut egui::Ui,
    rect: Rect,
    available: bool,
    listening: bool,
    with_text: bool,
) -> Action {
    let (name, text, glyph, hint) = if listening {
        (
            "Stop listening",
            "Stop",
            Glyph::Stop,
            "Stop listening. L plays or stops the pattern.",
        )
    } else {
        (
            "Listen",
            "Listen",
            Glyph::Play,
            "Listen to the pattern. L plays or stops it.",
        )
    };
    let response = key(
        ui,
        rect,
        Key {
            id: "listen",
            glyph,
            enabled: available || listening,
            ..Key::new(name, if with_text { text } else { "" })
        },
    );
    let response = if response.enabled() {
        response.hover_hint(hint)
    } else {
        response.disabled_hint(NO_SOURCE)
    };
    match (response.clicked(), listening) {
        (true, false) => Action::Listen,
        (true, true) => Action::Stop,
        _ => Action::None,
    }
}

/// A shared text button, such as Import details.
pub(super) fn button(
    ui: &mut egui::Ui,
    rect: Rect,
    text: &str,
    enabled: bool,
    primary: bool,
) -> Response {
    controls::button_at(
        ui,
        rect,
        text,
        text,
        if primary {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        },
        enabled,
    )
}
