//! The three application themes. A theme is one set of roles that the shared
//! renderers read, the material that they paint, and the theme's type faces.
//! Studio paints flat matte surfaces, Aero molded plastic, and Vintage painted
//! metal. Layout, control positions, behavior, and features never depend on
//! the theme. Surfaces are painted in code; Vintage alone reuses the approved
//! Folded Metal and ivory mixer artwork that it was drawn from.
use eframe::egui::{self, Color32, CornerRadius, Pos2, Rect, Stroke, Vec2};
pub(super) use mixjam_techno::application::options::Theme;
use std::cell::Cell;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Material {
    /// Matte paint: flat fills, hairline edges, small radii.
    Flat,
    /// Molded plastic: soft vertical shading, restrained rims, round shapes.
    Plastic,
    /// Painted metal: the Folded Metal artwork, bevels, and enamel.
    Metal,
}

/// Every color that a renderer chooses by meaning. Renderers never name a
/// theme; they ask for a role and, where shape differs, for the material.
pub(super) struct Roles {
    pub material: Material,
    // The workspace: header, transport, arrangement, browser, and menus.
    pub canvas: Color32,
    pub panel: Color32,
    pub raised: Color32,
    pub hover: Color32,
    /// Sound rows, at rest and hovered or selected. They carry small text
    /// under a category tint, so plastic needs them darker than `raised`.
    pub tile: Color32,
    pub tile_hover: Color32,
    pub line: Color32,
    pub text: Color32,
    pub muted: Color32,
    pub accent: Color32,
    pub coral: Color32,
    pub warning: Color32,
    pub selection: Color32,
    pub drag_label: Color32,
    pub row_alternate: Color32,
    pub bar_line: Color32,
    /// Text on light fills such as clips and the accent.
    pub on_light: Color32,
    pub sections: [Color32; 6],
    // Tool chassis, floating windows, and boards.
    pub ink: Color32,
    /// Printed text on a chassis.
    pub legend: Color32,
    /// Bays, value faces, and secondary button faces.
    pub face: Color32,
    pub help: Color32,
    pub primary: Color32,
    pub disabled_face: Color32,
    pub disabled_text: Color32,
    /// Slider caps and switch knobs.
    pub cap: Color32,
    pub danger: Color32,
    pub bay_edge: Color32,
    pub well_edge: Color32,
    pub widget_edge: Color32,
    pub widget_hover: Color32,
    /// Chassis body and accent per tool palette: Utility, Marine, Granite,
    /// Red, Olive, and Indigo.
    pub tools: [(Color32, Color32); 6],
    // The mixer.
    pub strip: Color32,
    pub return_strip: Color32,
    pub mixer_floor: Color32,
    /// Printed text and scale marks on a track strip.
    pub strip_text: Color32,
    pub strip_edge: Color32,
    pub strip_face: Color32,
    /// A selected strip button.
    pub strip_lit: Color32,
    pub strip_slot: Color32,
    /// The keyboard focus ring on a track strip.
    pub strip_focus: Color32,
    // Lamps and meters.
    pub lamp_on: Color32,
    pub lamp_warn: Color32,
    pub lamp_peak: Color32,
    pub lamp_good: Color32,
    pub lamp_off: Color32,
    pub meter_face: Color32,
    // Editing grids.
    pub grid_line: Color32,
    pub cell_edge: Color32,
    pub cell_hover: Color32,
    pub cell_alternate: Color32,
    pub tool_edge: Color32,
}

const fn rgb(r: u8, g: u8, b: u8) -> Color32 {
    Color32::from_rgb(r, g, b)
}

/// Section bookmark colors, in section order. They name sections, so every
/// theme keeps them.
const SECTIONS: [Color32; 6] = [
    rgb(242, 190, 92),
    rgb(110, 205, 214),
    rgb(196, 150, 245),
    rgb(240, 132, 180),
    rgb(130, 176, 250),
    rgb(236, 150, 104),
];

static STUDIO: Roles = Roles {
    material: Material::Flat,
    canvas: rgb(21, 23, 26),
    panel: rgb(28, 31, 35),
    raised: rgb(40, 44, 49),
    hover: rgb(52, 57, 63),
    tile: rgb(40, 44, 49),
    tile_hover: rgb(52, 57, 63),
    line: rgb(60, 66, 73),
    text: rgb(236, 239, 241),
    muted: rgb(164, 174, 182),
    accent: rgb(190, 232, 96),
    coral: rgb(255, 130, 130),
    warning: rgb(232, 197, 133),
    selection: rgb(62, 82, 40),
    drag_label: rgb(245, 220, 146),
    row_alternate: rgb(25, 28, 31),
    bar_line: rgb(36, 40, 45),
    on_light: rgb(21, 23, 26),
    sections: SECTIONS,
    ink: rgb(12, 13, 15),
    legend: rgb(234, 237, 239),
    face: rgb(30, 33, 37),
    help: rgb(170, 178, 185),
    primary: rgb(190, 232, 96),
    disabled_face: rgb(46, 49, 53),
    disabled_text: rgb(176, 182, 188),
    cap: rgb(205, 209, 213),
    danger: rgb(160, 54, 44),
    bay_edge: rgb(56, 61, 67),
    well_edge: rgb(98, 105, 113),
    widget_edge: rgb(76, 82, 89),
    widget_hover: rgb(48, 52, 57),
    tools: [
        (rgb(42, 46, 51), rgb(236, 152, 58)),
        (rgb(34, 47, 52), rgb(92, 192, 202)),
        (rgb(45, 47, 50), rgb(180, 190, 194)),
        (rgb(54, 39, 39), rgb(240, 126, 112)),
        (rgb(41, 47, 35), rgb(170, 198, 100)),
        (rgb(39, 41, 58), rgb(158, 172, 240)),
    ],
    strip: rgb(36, 39, 44),
    return_strip: rgb(29, 32, 36),
    mixer_floor: rgb(24, 26, 30),
    strip_text: rgb(232, 235, 237),
    strip_edge: rgb(62, 68, 75),
    strip_face: rgb(22, 24, 27),
    strip_lit: rgb(236, 152, 58),
    strip_slot: rgb(10, 11, 13),
    strip_focus: rgb(190, 232, 96),
    lamp_on: rgb(190, 232, 96),
    lamp_warn: rgb(246, 178, 64),
    lamp_peak: rgb(232, 72, 56),
    lamp_good: rgb(110, 196, 112),
    lamp_off: rgb(46, 50, 54),
    meter_face: rgb(226, 229, 231),
    grid_line: rgb(96, 104, 112),
    cell_edge: rgb(86, 94, 102),
    cell_hover: rgb(52, 58, 64),
    cell_alternate: rgb(30, 33, 37),
    tool_edge: rgb(98, 105, 113),
};

static AERO: Roles = Roles {
    material: Material::Plastic,
    canvas: rgb(20, 64, 132),
    panel: rgb(11, 36, 80),
    raised: rgb(30, 82, 156),
    hover: rgb(44, 102, 180),
    tile: rgb(20, 56, 114),
    tile_hover: rgb(26, 66, 130),
    line: rgb(62, 118, 190),
    text: rgb(242, 248, 255),
    muted: rgb(188, 212, 240),
    accent: rgb(150, 236, 90),
    coral: rgb(255, 146, 146),
    warning: rgb(255, 214, 140),
    selection: rgb(28, 104, 150),
    drag_label: rgb(255, 230, 150),
    row_alternate: rgb(14, 42, 90),
    bar_line: rgb(24, 56, 108),
    on_light: rgb(8, 28, 62),
    sections: SECTIONS,
    ink: rgb(3, 14, 34),
    legend: rgb(246, 250, 255),
    face: rgb(10, 36, 82),
    help: rgb(190, 214, 242),
    primary: rgb(120, 214, 250),
    disabled_face: rgb(50, 76, 116),
    disabled_text: rgb(196, 210, 228),
    cap: rgb(232, 240, 250),
    danger: rgb(172, 40, 50),
    bay_edge: rgb(40, 92, 160),
    well_edge: rgb(100, 154, 218),
    widget_edge: rgb(72, 128, 198),
    widget_hover: rgb(26, 70, 132),
    tools: [
        (rgb(24, 70, 140), rgb(255, 196, 84)),
        (rgb(14, 106, 132), rgb(128, 234, 242)),
        (rgb(62, 84, 116), rgb(196, 218, 242)),
        (rgb(170, 56, 50), rgb(255, 184, 164)),
        (rgb(52, 112, 42), rgb(196, 242, 126)),
        (rgb(62, 64, 168), rgb(184, 190, 255)),
    ],
    strip: rgb(28, 78, 152),
    return_strip: rgb(20, 58, 120),
    mixer_floor: rgb(14, 44, 96),
    strip_text: rgb(240, 246, 255),
    strip_edge: rgb(76, 136, 210),
    strip_face: rgb(9, 32, 74),
    strip_lit: rgb(150, 236, 90),
    strip_slot: rgb(4, 16, 40),
    strip_focus: rgb(128, 234, 255),
    lamp_on: rgb(150, 236, 90),
    lamp_warn: rgb(255, 196, 84),
    lamp_peak: rgb(255, 86, 86),
    lamp_good: rgb(120, 222, 124),
    lamp_off: rgb(22, 50, 92),
    meter_face: rgb(226, 240, 255),
    grid_line: rgb(70, 122, 192),
    cell_edge: rgb(62, 112, 182),
    cell_hover: rgb(36, 84, 152),
    cell_alternate: rgb(14, 44, 92),
    tool_edge: rgb(100, 154, 218),
};

static VINTAGE: Roles = Roles {
    material: Material::Metal,
    canvas: rgb(24, 23, 21),
    panel: rgb(31, 30, 27),
    raised: rgb(45, 43, 39),
    hover: rgb(58, 55, 49),
    tile: rgb(45, 43, 39),
    tile_hover: rgb(58, 55, 49),
    line: rgb(76, 72, 64),
    text: rgb(242, 233, 210),
    muted: rgb(184, 174, 152),
    accent: rgb(242, 170, 62),
    coral: rgb(250, 132, 108),
    warning: rgb(240, 202, 132),
    selection: rgb(92, 68, 32),
    drag_label: rgb(250, 222, 150),
    row_alternate: rgb(27, 26, 24),
    bar_line: rgb(40, 38, 34),
    on_light: rgb(26, 24, 20),
    sections: SECTIONS,
    ink: rgb(18, 20, 18),
    legend: rgb(249, 242, 220),
    face: rgb(35, 38, 36),
    help: rgb(190, 185, 166),
    primary: rgb(212, 205, 182),
    disabled_face: rgb(61, 64, 60),
    disabled_text: rgb(201, 203, 192),
    cap: rgb(190, 190, 185),
    danger: rgb(131, 54, 39),
    bay_edge: rgb(70, 72, 64),
    well_edge: rgb(131, 134, 126),
    widget_edge: rgb(93, 91, 82),
    widget_hover: rgb(58, 59, 55),
    tools: [
        (rgb(60, 65, 64), rgb(231, 147, 49)),
        (rgb(32, 83, 91), rgb(85, 174, 183)),
        (rgb(52, 57, 60), rgb(173, 186, 189)),
        (rgb(154, 45, 39), rgb(235, 116, 104)),
        (rgb(60, 68, 40), rgb(157, 178, 94)),
        (rgb(46, 54, 96), rgb(150, 166, 236)),
    ],
    strip: rgb(201, 194, 175),
    return_strip: rgb(29, 33, 34),
    mixer_floor: rgb(29, 33, 34),
    strip_text: rgb(28, 29, 26),
    strip_edge: rgb(108, 106, 97),
    strip_face: rgb(29, 33, 34),
    strip_lit: rgb(245, 193, 86),
    strip_slot: rgb(11, 14, 14),
    strip_focus: rgb(28, 29, 26),
    lamp_on: rgb(242, 170, 62),
    lamp_warn: rgb(246, 178, 64),
    lamp_peak: rgb(232, 72, 56),
    lamp_good: rgb(110, 196, 112),
    lamp_off: rgb(44, 48, 42),
    meter_face: rgb(240, 228, 190),
    grid_line: rgb(120, 128, 108),
    cell_edge: rgb(108, 116, 98),
    cell_hover: rgb(62, 68, 53),
    cell_alternate: rgb(29, 33, 28),
    tool_edge: rgb(100, 105, 101),
};

thread_local! {
    /// The theme that this thread paints. The UI paints on one thread; each
    /// test thread starts in Studio, the default.
    static CURRENT: Cell<Theme> = const { Cell::new(Theme::Studio) };
}

pub(super) fn current() -> Theme {
    CURRENT.with(Cell::get)
}

pub(super) fn roles() -> &'static Roles {
    of(current())
}

/// Paint `theme` on this thread without restyling a context. `adopt` uses it,
/// and tests that only paint or compare colors.
pub(super) fn paint_as(theme: Theme) {
    CURRENT.with(|current| current.set(theme));
}

pub(super) fn of(theme: Theme) -> &'static Roles {
    match theme {
        Theme::Studio => &STUDIO,
        Theme::Aero => &AERO,
        Theme::Vintage => &VINTAGE,
    }
}

/// Paint `theme` from now on. The style and fonts change once, when the theme
/// differs from the one that `ctx` last adopted; painting stays static.
pub(super) fn adopt(ctx: &egui::Context, theme: Theme) {
    let id = egui::Id::new("adopted-theme");
    paint_as(theme);
    if ctx.data(|data| data.get_temp::<Theme>(id)) != Some(theme) {
        super::configure_style(ctx);
        ctx.data_mut(|data| data.insert_temp(id, theme));
        ctx.request_repaint();
    }
}

impl Roles {
    /// A corner radius in the theme's shape language for a `base` radius:
    /// tight on Studio, round on Aero, and as drawn on Vintage.
    pub fn corner(&self, base: f32) -> f32 {
        match self.material {
            Material::Flat => (base * 0.5).min(4.0),
            Material::Plastic => (base * 1.6).min(14.0),
            Material::Metal => base,
        }
    }
}

/// The family of headings and window titles in the current theme.
pub(super) fn heading_family() -> egui::FontFamily {
    egui::FontFamily::Name(HEADING.into())
}

/// The family of the product name, the same in every theme.
pub(super) fn brand_family() -> egui::FontFamily {
    egui::FontFamily::Name(BRAND.into())
}

/// Every heading row is this many ems high in every theme: the line box of
/// Syncopate, which the shared layout was measured with. Heading faces have
/// taller natural line boxes, which would move the layout.
pub(super) const HEADING_LINE: f32 = 1.041;

const HEADING: &str = "Heading";
const BRAND: &str = "Brand";

struct Face {
    name: &'static str,
    bytes: &'static [u8],
    /// Scales the face so its headings fill the room that the original title
    /// face was measured for.
    scale: f32,
    /// Raises the glyphs, in ems, so figures and capitals center in their
    /// row: for a text face its natural row, as Inter's do, and for a heading
    /// face the pinned `HEADING_LINE` row. Each value is measured from the
    /// face's line metrics and the bounds of its "0" or "H".
    rise: f32,
}

const fn face(name: &'static str, bytes: &'static [u8], scale: f32, rise: f32) -> Face {
    Face {
        name,
        bytes,
        scale,
        rise,
    }
}

const SYNCOPATE: Face = face(
    "Syncopate Bold",
    include_bytes!("../../assets/fonts/Syncopate-Bold.ttf"),
    1.0,
    0.0,
);
const INTER: Face = face(
    "Inter Medium",
    include_bytes!("../../assets/fonts/Inter-Medium.ttf"),
    1.0,
    0.0,
);

/// The text face and the heading face of `theme`.
fn faces(theme: Theme) -> [Face; 2] {
    match theme {
        // Plex Sans text would need a 1.3 em line box; Inter keeps the shared
        // layout's 1.21 em rows.
        Theme::Studio => [
            INTER,
            face(
                "IBM Plex Sans SemiBold",
                include_bytes!("../../assets/fonts/IBMPlexSans-SemiBold.ttf"),
                1.0,
                0.1555,
            ),
        ],
        Theme::Aero => [
            face(
                "Fira Sans Medium",
                include_bytes!("../../assets/fonts/FiraSans-Medium.ttf"),
                1.0,
                0.0,
            ),
            face(
                "Michroma",
                include_bytes!("../../assets/fonts/Michroma-Regular.ttf"),
                0.86,
                0.176,
            ),
        ],
        Theme::Vintage => [
            face(
                "Barlow Medium",
                include_bytes!("../../assets/fonts/Barlow-Medium.ttf"),
                1.0,
                0.05,
            ),
            face(
                "Barlow Condensed SemiBold",
                include_bytes!("../../assets/fonts/BarlowCondensed-SemiBold.ttf"),
                1.2,
                0.2162,
            ),
        ],
    }
}
/// Embed only the current theme's faces, the brand face, and Inter, which
/// covers symbols that a theme face lacks.
pub(super) fn configure_fonts(ctx: &egui::Context) {
    let mut fonts = egui::FontDefinitions::default();
    let fallback = fonts.families[&egui::FontFamily::Proportional].clone();
    let mut add = |face: &Face| {
        fonts.font_data.insert(
            face.name.into(),
            egui::FontData::from_static(face.bytes)
                .tweak(egui::FontTweak {
                    scale: face.scale,
                    y_offset_factor: -face.rise,
                    ..Default::default()
                })
                .into(),
        );
        face.name.to_owned()
    };
    let [text, heading] = faces(current());
    let chain = |first: String, inter: &str| {
        let mut names = vec![first];
        if names[0] != inter {
            names.push(inter.to_owned());
        }
        names.extend(fallback.iter().cloned());
        names
    };
    let inter = add(&INTER);
    let text = chain(add(&text), &inter);
    let heading = chain(add(&heading), &inter);
    let brand = chain(add(&SYNCOPATE), &inter);
    fonts
        .families
        .insert(egui::FontFamily::Proportional, text.clone());
    fonts.families.insert(egui::FontFamily::Monospace, text);
    fonts
        .families
        .insert(egui::FontFamily::Name(HEADING.into()), heading);
    fonts
        .families
        .insert(egui::FontFamily::Name(BRAND.into()), brand);
    ctx.set_fonts(fonts);
}

/// Points around a rounded rectangle, clockwise from its top edge, each with
/// its outward unit normal.
fn rounded_outline(rect: Rect, radius: f32) -> Vec<(Pos2, Vec2)> {
    const STEPS: usize = 6;
    let radius = radius
        .min(rect.width() / 2.0)
        .min(rect.height() / 2.0)
        .max(0.0);
    let corners = [
        (rect.right_top() + Vec2::new(-radius, radius), -90.0_f32),
        (rect.right_bottom() + Vec2::new(-radius, -radius), 0.0),
        (rect.left_bottom() + Vec2::new(radius, -radius), 90.0),
        (rect.left_top() + Vec2::new(radius, radius), 180.0),
    ];
    let mut points = Vec::with_capacity(4 * (STEPS + 1));
    for (center, start) in corners {
        for step in 0..=STEPS {
            let normal = Vec2::angled((start + 90.0 * step as f32 / STEPS as f32).to_radians());
            points.push((center + normal * radius, normal));
        }
    }
    points
}

/// A rounded rectangle shaded from `top` to `bottom`. A vertical gradient is
/// linear in y, so the fan's interpolation reproduces it exactly. Like egui's
/// own shapes, the silhouette fades over one physical pixel; a hard mesh
/// edge would show its stairs through the antialiased rim drawn over it.
pub(super) fn gradient(
    painter: &egui::Painter,
    rect: Rect,
    radius: f32,
    top: Color32,
    bottom: Color32,
) {
    if rect.width() <= 0.0 || rect.height() <= 0.0 {
        return;
    }
    let shade = |point: Pos2| {
        top.lerp_to_gamma(
            bottom,
            ((point.y - rect.top()) / rect.height()).clamp(0.0, 1.0),
        )
    };
    let mut mesh = egui::Mesh::default();
    mesh.colored_vertex(rect.center(), shade(rect.center()));
    let feather = 0.5 / painter.pixels_per_point();
    let outline = rounded_outline(rect, radius);
    let count = outline.len() as u32;
    for (point, normal) in outline {
        let inner = point - normal * feather;
        mesh.colored_vertex(inner, shade(inner));
        mesh.colored_vertex(point + normal * feather, Color32::TRANSPARENT);
    }
    for index in 0..count {
        let next = (index + 1) % count;
        let (inner, outer) = (1 + 2 * index, 2 + 2 * index);
        let (next_inner, next_outer) = (1 + 2 * next, 2 + 2 * next);
        mesh.add_triangle(0, inner, next_inner);
        mesh.add_triangle(inner, outer, next_outer);
        mesh.add_triangle(inner, next_outer, next_inner);
    }
    painter.add(mesh);
}

/// How far molded plastic lightens toward its top and darkens toward its foot.
/// Printed ink keeps its contrast at the foot.
pub(super) const PLASTIC_LIGHT: f32 = 0.06;
pub(super) const PLASTIC_SHADE: f32 = 0.10;
/// The white of the gloss band over a plastic surface's upper part.
pub(super) const PLASTIC_GLOSS: u8 = 10;

/// Molded plastic: broad soft shading, a quiet highlight across the upper
/// part, and a restrained rim. The finish is the owner's softer gloss.
pub(super) fn plastic(painter: &egui::Painter, rect: Rect, radius: f32, fill: Color32) {
    // egui snaps the rim strokes below to whole pixels; the fill snaps with
    // them and ends under the rim, as egui's own filled and stroked shapes
    // do. A fill reaching the rim's outer edge tints its antialiased pixels.
    let rect = egui::emath::GuiRounding::round_to_pixels(rect, painter.pixels_per_point());
    gradient(
        painter,
        rect.shrink(0.5),
        (radius - 0.5).max(0.0),
        fill.lerp_to_gamma(Color32::WHITE, PLASTIC_LIGHT),
        fill.lerp_to_gamma(Color32::BLACK, PLASTIC_SHADE),
    );
    let gloss = Rect::from_min_max(
        rect.min + Vec2::new(2.0, 1.5),
        Pos2::new(rect.right() - 2.0, rect.top() + rect.height() * 0.45),
    );
    if gloss.height() > 2.0 && gloss.width() > 4.0 {
        let round = (radius - 1.5).max(0.0) as u8;
        painter.rect_filled(
            gloss,
            CornerRadius {
                nw: round,
                ne: round,
                sw: 2,
                se: 2,
            },
            Color32::from_white_alpha(PLASTIC_GLOSS),
        );
    }
    painter.rect_stroke(
        rect.shrink(1.0),
        (radius - 1.0).max(0.0),
        Stroke::new(
            1.0_f32,
            fill.lerp_to_gamma(Color32::WHITE, 0.3).gamma_multiply(0.6),
        ),
        egui::StrokeKind::Inside,
    );
    painter.rect_stroke(
        rect,
        radius,
        Stroke::new(1.0_f32, fill.lerp_to_gamma(Color32::BLACK, 0.5)),
        egui::StrokeKind::Inside,
    );
}

/// A raised surface in the theme's material, for buttons, clips, and strips.
pub(super) fn surface(painter: &egui::Painter, rect: Rect, radius: f32, fill: Color32) {
    match roles().material {
        Material::Flat => {
            painter.rect_filled(rect, radius, fill);
        }
        Material::Plastic => plastic(painter, rect, radius, fill),
        Material::Metal => {
            painter.rect_filled(rect, radius, fill);
            // Painted sheet metal: light catches the top fold, the lower
            // edge falls into shade.
            painter.line_segment(
                [
                    rect.left_top() + Vec2::new(radius.max(2.0), 1.0),
                    rect.right_top() + Vec2::new(-radius.max(2.0), 1.0),
                ],
                Stroke::new(1.0_f32, Color32::from_white_alpha(40)),
            );
            painter.line_segment(
                [
                    rect.left_bottom() + Vec2::new(radius.max(2.0), -1.0),
                    rect.right_bottom() + Vec2::new(-radius.max(2.0), -1.0),
                ],
                Stroke::new(1.0_f32, Color32::from_black_alpha(60)),
            );
        }
    }
}

/// An arc around `center` from `from` to `to` degrees, clockwise from east.
pub(super) fn arc(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    from: f32,
    to: f32,
    stroke: Stroke,
) {
    let steps = (((to - from).abs() / 8.0).ceil() as usize).max(2);
    let points = (0..=steps)
        .map(|step| {
            let angle = (from + (to - from) * step as f32 / steps as f32).to_radians();
            center + Vec2::angled(angle) * radius
        })
        .collect();
    painter.add(egui::Shape::line(points, stroke));
}

/// A rotary face drawn in code. `fraction` turns the pointer through 270
/// degrees from lower left; `accent` marks the value on Studio and colors the
/// cap on Aero. `on_light` marks a light surface under the control.
pub(super) fn dial(
    painter: &egui::Painter,
    center: Pos2,
    radius: f32,
    fraction: f32,
    accent: Color32,
    on_light: bool,
) {
    let roles = roles();
    let start = -225.0;
    let end = start + 270.0 * fraction.clamp(0.0, 1.0);
    let pointer = Vec2::angled(end.to_radians());
    match roles.material {
        Material::Flat => {
            let mark = if on_light { roles.ink } else { roles.legend };
            let value = if on_light { roles.ink } else { accent };
            arc(
                painter,
                center,
                radius - 2.0,
                start,
                start + 270.0,
                Stroke::new(3.0_f32, mark.gamma_multiply(0.22)),
            );
            if end > start {
                arc(
                    painter,
                    center,
                    radius - 2.0,
                    start,
                    end,
                    Stroke::new(3.0_f32, value),
                );
            }
            painter.circle_filled(center, radius * 0.62, roles.face);
            painter.circle_stroke(center, radius * 0.62, Stroke::new(1.0_f32, roles.bay_edge));
            painter.line_segment(
                [
                    center + pointer * radius * 0.12,
                    center + pointer * radius * 0.56,
                ],
                Stroke::new(2.0_f32, roles.legend),
            );
        }
        Material::Plastic | Material::Metal => {
            // A molded skirt, a colored cap shaded toward the light, and a
            // soft highlight. Metal uses its bitmap knob instead; this face
            // stands in when a caller asks for a code dial there.
            painter.circle_filled(
                center + Vec2::new(0.0, 1.5),
                radius,
                Color32::from_black_alpha(70),
            );
            painter.circle_filled(center, radius, roles.ink);
            let cap = radius - 2.5;
            for step in 0..6 {
                let share = step as f32 / 5.0;
                painter.circle_filled(
                    center - Vec2::new(0.0, cap * 0.18 * share),
                    cap * (1.0 - 0.45 * share),
                    accent
                        .lerp_to_gamma(Color32::BLACK, 0.28)
                        .lerp_to_gamma(Color32::WHITE, 0.32 * share),
                );
            }
            painter.circle_stroke(
                center,
                cap,
                Stroke::new(1.0_f32, accent.lerp_to_gamma(Color32::BLACK, 0.5)),
            );
            painter.circle_filled(
                center + Vec2::new(-cap * 0.3, -cap * 0.38),
                cap * 0.22,
                Color32::from_white_alpha(70),
            );
            painter.line_segment(
                [center + pointer * cap * 0.25, center + pointer * cap * 0.88],
                Stroke::new(2.5_f32, roles.ink),
            );
        }
    }
}

/// A fader cap in code. Vintage paints its bitmap cap instead.
pub(super) fn fader_cap(painter: &egui::Painter, center: Pos2, size: Vec2) {
    let roles = roles();
    let rect = Rect::from_center_size(center, size);
    match roles.material {
        Material::Flat => {
            painter.rect_filled(rect.translate(Vec2::new(0.0, 1.0)), 2.0, roles.ink);
            painter.rect_filled(rect, 2.0, roles.cap);
            painter.line_segment(
                [
                    rect.left_center() + Vec2::new(3.0, 0.0),
                    rect.right_center() - Vec2::new(3.0, 0.0),
                ],
                Stroke::new(2.0_f32, roles.ink),
            );
        }
        Material::Plastic | Material::Metal => {
            painter.rect_filled(
                rect.translate(Vec2::new(0.0, 2.0)),
                6.0,
                Color32::from_black_alpha(90),
            );
            plastic(painter, rect, 6.0, roles.cap);
            painter.line_segment(
                [
                    rect.left_center() + Vec2::new(4.0, 0.0),
                    rect.right_center() - Vec2::new(4.0, 0.0),
                ],
                Stroke::new(2.0_f32, roles.ink),
            );
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn contrast(a: Color32, b: Color32) -> f32 {
        super::super::controls::contrast(a, b)
    }

    /// The lightest and darkest colors a surface of `fill` shows. Plastic
    /// lightens its top, glosses its upper part, and shades its foot; the
    /// other materials keep text on the flat fill.
    fn painted(r: &Roles, fill: Color32) -> Vec<Color32> {
        if r.material != Material::Plastic {
            return vec![fill];
        }
        let top = fill.lerp_to_gamma(Color32::WHITE, PLASTIC_LIGHT);
        let gloss = |c: u8| c + ((255 - c) as u32 * PLASTIC_GLOSS as u32 / 255) as u8;
        vec![
            Color32::from_rgb(gloss(top.r()), gloss(top.g()), gloss(top.b())),
            fill.lerp_to_gamma(Color32::BLACK, PLASTIC_SHADE),
        ]
    }

    /// The lowest contrast of `text` across a painted surface of `fill`.
    fn painted_contrast(r: &Roles, text: Color32, fill: Color32) -> f32 {
        painted(r, fill)
            .into_iter()
            .map(|shade| contrast(text, shade))
            .fold(f32::INFINITY, f32::min)
    }

    #[test]
    fn text_on_painted_surfaces_keeps_its_contrast() {
        for theme in Theme::ALL {
            let r = of(theme);
            let name = theme.label();
            let mut pairs = vec![
                ("text on a raised widget", r.text, r.raised, 4.5),
                ("legend on danger", r.legend, r.danger, 4.5),
                ("strip text", r.strip_text, r.strip, 4.5),
                // Album Review's secondary text sits on the Utility chassis.
                ("muted on the Utility chassis", r.muted, r.tools[0].0, 4.5),
            ];
            for (body, _) in r.tools {
                pairs.push(("legend on a tool body", r.legend, body, 4.5));
                pairs.push(("help on a tool body", r.help, body, 3.0));
            }
            for (what, text, fill, minimum) in pairs {
                let ratio = painted_contrast(r, text, fill);
                assert!(ratio >= minimum, "{name} {what} {fill:?}: {ratio:.2}:1");
            }
            // A sound row adds its category tint, and its details line is
            // translucent.
            let over = |top: Color32, bottom: Color32, alpha: f32| {
                let mix =
                    |a: u8, b: u8| (a as f32 * alpha + b as f32 * (1.0 - alpha)).round() as u8;
                Color32::from_rgb(
                    mix(top.r(), bottom.r()),
                    mix(top.g(), bottom.g()),
                    mix(top.b(), bottom.b()),
                )
            };
            let groups = [
                "loop", "drum", "bass", "sequence", "sphere", "effect", "voice", "other",
            ];
            for fill in [r.tile, r.tile_hover] {
                for shade in painted(r, fill) {
                    for group in groups {
                        let [red, green, blue, alpha] =
                            super::super::view::tile_tint(super::super::view::group_color(group))
                                .to_srgba_unmultiplied();
                        let tile = over(
                            Color32::from_rgb(red, green, blue),
                            shade,
                            alpha as f32 / 255.0,
                        );
                        let details = over(r.text, tile, super::super::view::TILE_DETAIL_ALPHA);
                        for (what, text) in [("name", r.text), ("details", details)] {
                            let ratio = contrast(text, tile);
                            assert!(ratio >= 4.5, "{name} {group} tile {what}: {ratio:.2}:1");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn every_theme_keeps_text_and_state_contrast() {
        for theme in Theme::ALL {
            let r = of(theme);
            let name = theme.label();
            for (what, text, fill, minimum) in [
                ("text on canvas", r.text, r.canvas, 7.0),
                ("text on panel", r.text, r.panel, 7.0),
                ("text on raised", r.text, r.raised, 4.5),
                ("muted on panel", r.muted, r.panel, 4.5),
                ("muted on canvas", r.muted, r.canvas, 4.5),
                ("accent on panel", r.accent, r.panel, 3.0),
                ("ink on accent", r.on_light, r.accent, 4.5),
                ("coral on panel", r.coral, r.panel, 4.5),
                ("legend on face", r.legend, r.face, 4.5),
                ("help on face", r.help, r.face, 4.5),
                ("ink on primary", r.ink, r.primary, 4.5),
                ("disabled text", r.disabled_text, r.disabled_face, 4.5),
                ("legend on danger", r.legend, r.danger, 4.5),
                ("strip text", r.strip_text, r.strip, 4.5),
                ("strip legend on face", r.legend, r.strip_face, 4.5),
                ("lit strip button", r.ink, r.strip_lit, 4.5),
                ("strip focus", r.strip_focus, r.strip, 3.0),
                ("lamp on against off", r.lamp_on, r.lamp_off, 3.0),
                ("ink on meter", r.ink, r.meter_face, 7.0),
            ] {
                let ratio = contrast(text, fill);
                assert!(ratio >= minimum, "{name} {what}: {ratio:.2}:1");
            }
            for (index, (body, accent)) in r.tools.iter().enumerate() {
                assert!(
                    contrast(r.legend, *body) >= 4.5,
                    "{name} tool {index} legend"
                );
                assert!(
                    contrast(r.ink, *accent) >= 4.5,
                    "{name} tool {index} selection"
                );
                assert!(contrast(r.help, *body) >= 3.0, "{name} tool {index} help");
            }
        }
    }

    #[test]
    fn tool_bodies_name_their_palette_within_a_theme() {
        // The shared controls find a window's palette from its body color.
        for theme in Theme::ALL {
            let r = of(theme);
            for (index, (body, _)) in r.tools.iter().enumerate() {
                for (other, (next, _)) in r.tools.iter().enumerate() {
                    assert!(index == other || body != next, "{}", theme.label());
                }
                assert!(
                    index == 0 || (*body != r.canvas && *body != r.panel),
                    "{}",
                    theme.label()
                );
            }
        }
    }

    #[test]
    fn studio_is_the_default_and_adoption_restyles_only_on_change() {
        assert_eq!(current(), Theme::Studio);
        let ctx = egui::Context::default();
        for theme in [Theme::Aero, Theme::Aero, Theme::Vintage, Theme::Studio] {
            adopt(&ctx, theme);
            assert_eq!(current(), theme);
            assert_eq!(ctx.style().visuals.panel_fill, of(theme).canvas);
        }
    }
}
