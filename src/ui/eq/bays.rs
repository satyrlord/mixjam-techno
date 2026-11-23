//! Hardware bay panels, slope selectors, and compact dial grids for the
//! 3-theme Channel Strip window.
use super::super::master::{Dial, Format, compact_dial, decimal};
use super::super::*;
use super::Channel;
use mixjam_techno::application::{
    FILTER_RESONANCE, KnobRange, TRACK_COUNT, TRACK_MONO_BELOW, TRACK_WIDTH,
    automation::FilterSlope,
    compressor::CompressorParam,
    drive::DriveParam,
    ducker::DuckerParam,
    eq::{CutSlope, EqParam},
};

const COMPACT_WELL: f32 = super::super::master::COMPACT_WELL;
const EQ_CELL_W: f32 = 59.0;
const BAY_ROW_H: f32 = 86.0;
const COMP_CELL_W: f32 = 64.0;
const DUCK_CELL_W: f32 = 57.0;
const DRIVE_CELL_W: f32 = 57.0;

pub(super) struct EditFlags {
    pub turned: bool,
    pub switched: bool,
}

impl EditFlags {
    pub(super) fn merge(&mut self, other: Self) {
        self.turned |= other.turned;
        self.switched |= other.switched;
    }
}

struct ParamMeta<P> {
    param: P,
    suffix: &'static str,
    label: &'static str,
    hint: &'static str,
    format: Format,
}

const EQ_META: [ParamMeta<EqParam>; 12] = [
    ParamMeta {
        param: EqParam::LowCut,
        suffix: "low cut",
        label: "Low cut",
        hint: "High-pass corner; 20 Hz lets everything through.",
        format: Format::Hertz,
    },
    ParamMeta {
        param: EqParam::LowHz,
        suffix: "low frequency",
        label: "Low Hz",
        hint: "Low shelf frequency.",
        format: Format::Hertz,
    },
    ParamMeta {
        param: EqParam::LowDb,
        suffix: "low gain",
        label: "Low",
        hint: "Low shelf gain in dB.",
        format: Format::db(true),
    },
    ParamMeta {
        param: EqParam::MidHz,
        suffix: "mid frequency",
        label: "Mid Hz",
        hint: "Mid peak center frequency.",
        format: Format::Hertz,
    },
    ParamMeta {
        param: EqParam::MidDb,
        suffix: "mid gain",
        label: "Mid",
        hint: "Mid peak gain in dB.",
        format: Format::db(true),
    },
    ParamMeta {
        param: EqParam::MidQ,
        suffix: "mid Q",
        label: "Mid Q",
        hint: "Mid peak bandwidth Q.",
        format: Format::tenths(""),
    },
    ParamMeta {
        param: EqParam::Peak2Hz,
        suffix: "peak 2 frequency",
        label: "Pk2 Hz",
        hint: "Second peak center frequency.",
        format: Format::Hertz,
    },
    ParamMeta {
        param: EqParam::Peak2Db,
        suffix: "peak 2 gain",
        label: "Pk2",
        hint: "Second peak gain in dB.",
        format: Format::db(true),
    },
    ParamMeta {
        param: EqParam::Peak2Q,
        suffix: "peak 2 Q",
        label: "Pk2 Q",
        hint: "Second peak bandwidth Q.",
        format: Format::tenths(""),
    },
    ParamMeta {
        param: EqParam::HighHz,
        suffix: "high frequency",
        label: "High Hz",
        hint: "High shelf frequency.",
        format: Format::Hertz,
    },
    ParamMeta {
        param: EqParam::HighDb,
        suffix: "high gain",
        label: "High",
        hint: "High shelf gain in dB.",
        format: Format::db(true),
    },
    ParamMeta {
        param: EqParam::HighCut,
        suffix: "high cut",
        label: "High cut",
        hint: "Low-pass corner; 20 kHz lets everything through.",
        format: Format::Hertz,
    },
];

const DRIVE_META: [ParamMeta<DriveParam>; 5] = [
    ParamMeta {
        param: DriveParam::Amount,
        suffix: "amount",
        label: "Amount",
        hint: "How hard the copy is driven in dB.",
        format: Format::db(false),
    },
    ParamMeta {
        param: DriveParam::Character,
        suffix: "character",
        label: "Char",
        hint: "0% odd harmonics; 100% asymmetric even.",
        format: Format::whole("%"),
    },
    ParamMeta {
        param: DriveParam::Mix,
        suffix: "mix",
        label: "Mix",
        hint: "Wet/dry blend of added harmonics.",
        format: Format::whole("%"),
    },
    ParamMeta {
        param: DriveParam::Focus,
        suffix: "focus",
        label: "Focus",
        hint: "High-pass corner on the added harmonics.",
        format: Format::Hertz,
    },
    ParamMeta {
        param: DriveParam::Output,
        suffix: "output",
        label: "Output",
        hint: "Output trim after drive.",
        format: Format::db(true),
    },
];

const COMP_META: [ParamMeta<CompressorParam>; 6] = [
    ParamMeta {
        param: CompressorParam::Threshold,
        suffix: "threshold",
        label: "Thresh",
        hint: "Level where gain reduction begins.",
        format: Format::db(true),
    },
    ParamMeta {
        param: CompressorParam::Ratio,
        suffix: "ratio",
        label: "Ratio",
        hint: "Compression ratio.",
        format: Format::Unit {
            unit: ":1",
            decimals: 1,
            signed: false,
        },
    },
    ParamMeta {
        param: CompressorParam::Attack,
        suffix: "attack",
        label: "Attack",
        hint: "Attack time in ms.",
        format: Format::tenths("ms"),
    },
    ParamMeta {
        param: CompressorParam::Release,
        suffix: "release",
        label: "Release",
        hint: "Release time in ms.",
        format: Format::whole("ms"),
    },
    ParamMeta {
        param: CompressorParam::Makeup,
        suffix: "makeup",
        label: "Makeup",
        hint: "Post-compression makeup gain.",
        format: Format::db(true),
    },
    ParamMeta {
        param: CompressorParam::ScHpfHz,
        suffix: "sidechain HPF",
        label: "SC HPF",
        hint: "Sidechain high-pass cutoff; 20 Hz is off.",
        format: Format::Hertz,
    },
];

const DUCK_META: [ParamMeta<DuckerParam>; 4] = [
    ParamMeta {
        param: DuckerParam::Threshold,
        suffix: "threshold",
        label: "Thresh",
        hint: "Key level where ducking starts.",
        format: Format::db(true),
    },
    ParamMeta {
        param: DuckerParam::Depth,
        suffix: "depth",
        label: "Depth",
        hint: "Maximum gain reduction in dB.",
        format: Format::db(false),
    },
    ParamMeta {
        param: DuckerParam::Attack,
        suffix: "attack",
        label: "Attack",
        hint: "Duck attack time in ms.",
        format: Format::tenths("ms"),
    },
    ParamMeta {
        param: DuckerParam::Release,
        suffix: "release",
        label: "Release",
        hint: "Duck release time in ms.",
        format: Format::whole("ms"),
    },
];

fn dial_positions(x: f32, y: f32) -> [Pos2; 3] {
    [
        Pos2::new(x, y),
        Pos2::new(x, y + 32.0),
        Pos2::new(x, y + 51.0),
    ]
}

fn draw_dial_param(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    spec: Dial<'_>,
    raw: f32,
    enabled: bool,
) -> Option<f64> {
    let before = decimal(raw);
    let mut value = before;
    compact_dial(ui, positions, &mut value, spec, enabled, COMPACT_WELL);
    (value != before).then_some(value)
}

type ParamAccess<P, S> = (fn(P) -> KnobRange, fn(P, &S) -> f32, fn(P, &mut S, f32));

fn draw_grid<P: Copy, S>(
    ui: &mut egui::Ui,
    center: impl Fn(usize) -> Pos2,
    prefix: &str,
    params: &[ParamMeta<P>],
    settings: &mut S,
    enabled: bool,
    (range, get, set): ParamAccess<P, S>,
) -> bool {
    let mut turned = false;
    for (index, meta) in params.iter().enumerate() {
        let at = center(index);
        let name = format!("{prefix} {}", meta.suffix);
        if let Some(value) = draw_dial_param(
            ui,
            dial_positions(at.x, at.y),
            Dial {
                name: &name,
                label: meta.label,
                hint: meta.hint,
                range: range(meta.param),
                format: meta.format,
                choices: &[],
            },
            get(meta.param, settings),
            enabled,
        ) {
            set(meta.param, settings, value as f32);
            turned = true;
        }
    }
    turned
}

fn bay_header(
    ui: &mut egui::Ui,
    rect: Rect,
    title: &str,
    accent: Color32,
    power_name: &str,
    powered: &mut bool,
    hint: &str,
) -> bool {
    controls::instrument::bay(ui, rect, title, accent, false);
    let toggle_rect = Rect::from_min_size(
        Pos2::new(rect.left() + 10.0, rect.top() + 21.0),
        Vec2::new(72.0, 20.0),
    );
    controls::instrument::bat_toggle(ui, toggle_rect, power_name, "Power", powered, hint, false)
        .clicked()
}

fn slope_buttons(
    ui: &mut egui::Ui,
    rect: Rect,
    prefix: &str,
    hint: &str,
    is_24db: bool,
) -> Option<bool> {
    let mut picked = None;
    let btn_w = 54.0;
    let gap = 4.0;
    for (idx, (label, target_24)) in [("12 dB", false), ("24 dB", true)].into_iter().enumerate() {
        let btn_rect = Rect::from_min_size(
            Pos2::new(rect.left() + idx as f32 * (btn_w + gap), rect.top()),
            Vec2::new(btn_w, 20.0),
        );
        let a11y = format!("{prefix} {label}");
        let selected = is_24db == target_24;
        let role = if selected {
            controls::ButtonRole::Primary
        } else {
            controls::ButtonRole::Secondary
        };
        let resp = controls::button_at(ui, btn_rect, &a11y, label, role, true);
        resp.widget_info(|| {
            egui::WidgetInfo::selected(egui::WidgetType::RadioButton, true, selected, &a11y)
        });
        if resp.hover_hint(hint).clicked() && !selected {
            picked = Some(target_24);
        }
    }
    picked
}

/// Top full-width bay: 6-band `ChannelEq` + switchable `12 / 24 dB` cut slope.
pub(super) fn eq_bay(
    ui: &mut egui::Ui,
    rect: Rect,
    name: &str,
    channel: &mut Channel,
) -> EditFlags {
    let r = roles();
    let mut switched = bay_header(
        ui,
        rect,
        "EQ",
        r.sections[1],
        &format!("{name} power"),
        &mut channel.eq.on,
        "Toggle 6-band channel EQ.",
    );

    let slope_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 122.0, rect.top() + 21.0),
        Vec2::new(112.0, 20.0),
    );
    ui.painter().text(
        Pos2::new(slope_rect.left() - 6.0, slope_rect.center().y),
        Align2::RIGHT_CENTER,
        "Cuts",
        FontId::proportional(10.0),
        r.legend,
    );
    if let Some(is_24) = slope_buttons(
        ui,
        slope_rect,
        &format!("{name} cut slope"),
        "Select 12 dB/oct or 24 dB/oct high-pass & low-pass cut slope.",
        channel.eq.cut_slope == CutSlope::Slope24,
    ) {
        channel.eq.cut_slope = if is_24 {
            CutSlope::Slope24
        } else {
            CutSlope::Slope12
        };
        switched = true;
    }

    let left = rect.left() + (rect.width() - 12.0 * EQ_CELL_W) * 0.5;
    let y = rect.top() + 63.0;
    for col in [1.0, 3.0, 6.0, 9.0, 11.0] {
        let x = left + col * EQ_CELL_W;
        ui.painter().line_segment(
            [Pos2::new(x, y - 14.0), Pos2::new(x, y + 54.0)],
            Stroke::new(1.0_f32, r.bay_edge.gamma_multiply(0.55)),
        );
    }
    let enabled = channel.eq.on;
    let turned = draw_grid(
        ui,
        |col| Pos2::new(left + (col as f32 + 0.5) * EQ_CELL_W, y),
        name,
        &EQ_META,
        &mut channel.eq,
        enabled,
        (EqParam::range, EqParam::get, EqParam::set),
    );
    EditFlags { turned, switched }
}

/// Bottom-left bay: saturation `Drive` power + 5 dials.
pub(super) fn drive_bay(
    ui: &mut egui::Ui,
    rect: Rect,
    track_num: usize,
    channel: &mut Channel,
) -> EditFlags {
    let prefix = format!("Track {track_num} drive");
    let switched = bay_header(
        ui,
        rect,
        "Drive",
        roles().sections[5],
        &format!("{prefix} power"),
        &mut channel.drive.on,
        "Toggle track saturation drive.",
    );

    let left = rect.left() + (rect.width() - 3.0 * DRIVE_CELL_W) * 0.5;
    let enabled = channel.drive.on;
    let turned = draw_grid(
        ui,
        |index| grid_center(rect, left, DRIVE_CELL_W, 3, index),
        &prefix,
        &DRIVE_META,
        &mut channel.drive,
        enabled,
        (DriveParam::range, DriveParam::get, DriveParam::set),
    );
    EditFlags { turned, switched }
}

fn grid_center(rect: Rect, left: f32, cell_width: f32, columns: usize, index: usize) -> Pos2 {
    let column = index % columns;
    let row = index / columns;
    Pos2::new(
        left + (column as f32 + 0.5) * cell_width,
        rect.top() + 63.0 + row as f32 * BAY_ROW_H,
    )
}

/// Bottom-center bay: 6-knob `Compressor` (including `SC HPF`) + `Width` & LR4 `Mono <`.
pub(super) fn comp_stereo_bay(
    ui: &mut egui::Ui,
    rect: Rect,
    track_num: usize,
    channel: &mut Channel,
) -> EditFlags {
    let prefix = format!("Track {track_num} compressor");
    let switched = bay_header(
        ui,
        rect,
        "Compressor & Stereo",
        roles().sections[4],
        &format!("{prefix} power"),
        &mut channel.compressor.on,
        "Toggle track compressor.",
    );

    let left = rect.left() + (rect.width() - 4.0 * COMP_CELL_W) * 0.5;
    let enabled = channel.compressor.on;
    let mut turned = draw_grid(
        ui,
        |index| grid_center(rect, left, COMP_CELL_W, 4, index),
        &prefix,
        &COMP_META,
        &mut channel.compressor,
        enabled,
        (
            CompressorParam::range,
            CompressorParam::get,
            CompressorParam::set,
        ),
    );

    let row1_y = rect.top() + 63.0 + BAY_ROW_H;
    let width_name = format!("Track {track_num} width");
    if let Some(w) = draw_dial_param(
        ui,
        dial_positions(left + 2.5 * COMP_CELL_W, row1_y),
        Dial {
            name: &width_name,
            label: "Width",
            hint: "Stereo width from 0% mono to 200% wide.",
            range: TRACK_WIDTH,
            format: Format::whole("%"),
            choices: &[],
        },
        channel.width,
        true,
    ) {
        channel.width = w as f32;
        turned = true;
    }
    let mono_name = format!("Track {track_num} mono below");
    if let Some(m) = draw_dial_param(
        ui,
        dial_positions(left + 3.5 * COMP_CELL_W, row1_y),
        Dial {
            name: &mono_name,
            label: "Mono <",
            hint: "Sum low frequencies below this LR4 crossover to mono; 20 Hz is off.",
            range: TRACK_MONO_BELOW,
            format: Format::Hertz,
            choices: &[],
        },
        channel.mono_below_hz,
        true,
    ) {
        channel.mono_below_hz = m as f32;
        turned = true;
    }
    EditFlags { turned, switched }
}

/// Bottom-right bay: `Filter` slope/resonance + `Ducker` power/key/dials
/// (keeping `"Track N ducker release value"` as the final tab stop).
pub(super) fn filter_ducker_bay(
    ui: &mut egui::Ui,
    rect: Rect,
    track: usize,
    channel: &mut Channel,
) -> EditFlags {
    let track_num = track + 1;
    let prefix = format!("Track {track_num} ducker");
    let mut switched = bay_header(
        ui,
        rect,
        "Filter & Ducker",
        roles().sections[2],
        &format!("{prefix} power"),
        &mut channel.ducker.on,
        "Toggle key-triggered ducker.",
    );
    if switched && channel.ducker.on && channel.ducker.key_track() == track {
        channel.ducker.set_key(track, f64::from(channel.ducker.key));
    }

    let slope_rect = Rect::from_min_size(
        Pos2::new(rect.right() - 122.0, rect.top() + 21.0),
        Vec2::new(112.0, 20.0),
    );
    if let Some(is_24) = slope_buttons(
        ui,
        slope_rect,
        &format!("Track {track_num} filter"),
        "Select Filter automation slope: 12 dB/oct or 24 dB/oct.",
        channel.filter_slope == FilterSlope::Slope24,
    ) {
        channel.filter_slope = if is_24 {
            FilterSlope::Slope24
        } else {
            FilterSlope::Slope12
        };
        switched = true;
    }

    let turned = filter_and_ducker_dials(ui, rect, track, track_num, &prefix, channel);
    EditFlags { turned, switched }
}

fn filter_and_ducker_dials(
    ui: &mut egui::Ui,
    rect: Rect,
    track: usize,
    track_num: usize,
    prefix: &str,
    channel: &mut Channel,
) -> bool {
    let mut turned = false;
    let left = rect.left() + (rect.width() - 4.0 * DUCK_CELL_W) * 0.5;
    let row0 = rect.top() + 63.0;

    let flt_name = format!("Track {track_num} filter resonance");
    if let Some(res_val) = draw_dial_param(
        ui,
        dial_positions(left + 0.5 * DUCK_CELL_W, row0),
        Dial {
            name: &flt_name,
            label: "Flt Res",
            hint: "Filter lane resonance.",
            range: FILTER_RESONANCE,
            format: Format::whole("%"),
            choices: &[],
        },
        channel.filter_resonance,
        true,
    ) {
        channel.filter_resonance = res_val as f32;
        turned = true;
    }

    let key_range = KnobRange {
        low: 1.0,
        high: TRACK_COUNT as f32,
        default: 1.0,
        step: 1.0,
        logarithmic: false,
    };
    let key_name = format!("{prefix} key");
    if let Some(key_val) = draw_dial_param(
        ui,
        dial_positions(left + 1.5 * DUCK_CELL_W, row0),
        Dial {
            name: &key_name,
            label: "Key",
            hint: "Key track that triggers ducking.",
            range: key_range,
            format: Format::Key,
            choices: &[],
        },
        f32::from(channel.ducker.key),
        channel.ducker.on,
    ) {
        channel.ducker.set_key(track, key_val);
        turned = true;
    }

    let enabled = channel.ducker.on;
    turned |= draw_grid(
        ui,
        |index| grid_center(rect, left, DUCK_CELL_W, 4, index + 2),
        prefix,
        &DUCK_META,
        &mut channel.ducker,
        enabled,
        (DuckerParam::range, DuckerParam::get, DuckerParam::set),
    );
    turned
}
