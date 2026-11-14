//! Dials: how a knob's value reads in its window, the printed face of each
//! knob, and the rotary with its value window and label.
use super::*;

/// Bands 0 and 9 are shelves with a 12 dB range; the others reach 18 dB.
fn band_range_db(band: usize) -> f64 {
    if band == 0 || band == 9 { 12.0 } else { 18.0 }
}

fn band_db(band: usize, value: f64) -> f64 {
    let range = band_range_db(band);
    value / f64::from(EQ_BAND_MAX) * 2.0 * range - range
}

fn band_label(hz: f64) -> String {
    if hz >= 1000.0 {
        format!("{} kHz", hz / 1000.0)
    } else {
        format!("{hz} Hz")
    }
}

/// How a dial's value reads in its window.
#[derive(Clone, Copy)]
pub(crate) enum Format {
    /// The Booster pedals' 0 to 1000 positions.
    Plain,
    /// The gain of a Booster equaliser band, shown in decibels.
    Band(usize),
    /// A number in a unit.
    Unit {
        unit: &'static str,
        decimals: usize,
        signed: bool,
    },
    /// The tape speed, in inches per second.
    Speed,
    /// A frequency: hertz below 1 kHz, kilohertz from there.
    Hertz,
    /// A dynamics pedal's key: Own at 0, else the track's number.
    Key,
}
impl Format {
    pub(crate) const fn db(signed: bool) -> Self {
        Self::Unit {
            unit: "dB",
            decimals: 1,
            signed,
        }
    }
    pub(crate) const fn tenths(unit: &'static str) -> Self {
        Self::Unit {
            unit,
            decimals: 1,
            signed: false,
        }
    }
    pub(crate) const fn whole(unit: &'static str) -> Self {
        Self::Unit {
            unit,
            decimals: 0,
            signed: false,
        }
    }
    fn unit(self) -> &'static str {
        match self {
            Self::Plain | Self::Key => "",
            Self::Band(_) => "dB",
            Self::Unit { unit, .. } => unit,
            Self::Speed => "ips",
            Self::Hertz => "Hz",
        }
    }
    pub(super) fn show(self, value: f64) -> String {
        // Rounding can leave a negative zero.
        let value = value + 0.0;
        match self {
            Self::Plain => format!("{value:.0}"),
            Self::Band(band) => format!("{:+.1}", band_db(band, value) + 0.0),
            Self::Speed => format!("{value:.0} ips"),
            Self::Hertz if value >= 10_000.0 => format!("{:.1} kHz", value / 1000.0),
            Self::Hertz if value >= 1000.0 => format!("{:.2} kHz", value / 1000.0),
            Self::Hertz => format!("{value:.0} Hz"),
            Self::Key if value < 0.5 => "Own".into(),
            Self::Key => format!("{value:.0}"),
            Self::Unit {
                unit,
                decimals,
                signed,
            } => {
                let number = if signed {
                    format!("{value:+.decimals$}")
                } else {
                    format!("{value:.decimals$}")
                };
                match unit {
                    "" => number,
                    "%" | ":1" => format!("{number}{unit}"),
                    _ => format!("{number} {unit}"),
                }
            }
        }
    }
    /// A typed value, with or without its unit.
    pub(super) fn read(self, text: &str, high: f64) -> Option<f64> {
        let text = text.trim();
        if let Self::Key = self
            && text.eq_ignore_ascii_case("own")
        {
            return Some(0.0);
        }
        if let Self::Hertz = self {
            let lower = text.to_lowercase();
            let (number, scale) = match lower.strip_suffix("khz").or(lower.strip_suffix('k')) {
                Some(number) => (number, 1000.0),
                None => (lower.strip_suffix("hz").unwrap_or(&lower), 1.0),
            };
            return number
                .trim()
                .parse::<f64>()
                .ok()
                .map(|number| number * scale);
        }
        let unit = self.unit();
        let text = if !unit.is_empty() && text.to_lowercase().ends_with(&unit.to_lowercase()) {
            &text[..text.len() - unit.len()]
        } else {
            text
        };
        let number: f64 = text.trim().parse().ok()?;
        Some(match self {
            Self::Band(band) => {
                let range = band_range_db(band);
                ((number + range) / (2.0 * range) * high).round()
            }
            _ => number,
        })
    }
}

/// Everything a dial shows besides its value.
pub(crate) struct Dial<'a> {
    pub name: &'a str,
    pub label: &'a str,
    pub hint: &'a str,
    pub range: KnobRange,
    pub format: Format,
    pub choices: &'a [f64],
}

/// The printed label, accessible name, description, and value format of a knob.
pub(super) fn face(knob: Knob) -> (String, String, &'static str, Format) {
    let (label, name, hint, format) = match knob {
        Knob::BoosterBand(band) => {
            let hz = EQ_FREQUENCIES[band];
            return (
                band_label(hz),
                format!("Booster EQ {hz} Hz"),
                "The middle position is flat.",
                Format::Band(band),
            );
        }
        Knob::Trim => (
            "Trim",
            "Trim",
            "Gain into the board. The meter reads 0 VU at -18 dBFS, where the mastering pedals work best.",
            Format::db(true),
        ),
        Knob::BoosterDrive => ("Drive", "Booster compressor drive", "", Format::Plain),
        Knob::BoosterSpeed => ("Speed", "Booster compressor speed", "", Format::Plain),
        Knob::BoosterGain => ("Gain", "Booster compressor gain", "", Format::Plain),
        Knob::BoosterSpread => ("Width", "Booster width", "", Format::Plain),
        Knob::SoftClipAmount => (
            "Amount",
            "Soft clip amount",
            "How far below the ceiling the curve starts to round peaks off.",
            Format::db(false),
        ),
        Knob::SoftClipCeiling => (
            "Ceiling",
            "Soft clip ceiling",
            "The level that rounded peaks approach.",
            Format::db(false),
        ),
        Knob::TubeDrive => (
            "Drive",
            "Tube drive",
            "How hard the mix drives the tube curve.",
            Format::tenths(""),
        ),
        Knob::TubeMix => (
            "Mix",
            "Tube mix",
            "The saturated share of the output.",
            Format::whole("%"),
        ),
        Knob::TrimEqHighPass => (
            "High-pass",
            "Trim EQ high-pass",
            "Removes rumble below this frequency.",
            Format::whole("Hz"),
        ),
        Knob::TrimEqMud => (
            "Mud 250",
            "Trim EQ mud",
            "A narrow cut at 250 Hz.",
            Format::db(false),
        ),
        Knob::TrimEqHarsh => (
            "Harsh 3.5k",
            "Trim EQ harsh",
            "A narrow cut at 3.5 kHz.",
            Format::db(false),
        ),
        Knob::CompressorThreshold => (
            "Threshold",
            "Bus compressor threshold",
            "Compression starts above this level.",
            Format::db(false),
        ),
        Knob::CompressorRatio => (
            "Ratio",
            "Bus compressor ratio",
            "How strongly the level above the threshold comes down.",
            Format::tenths(":1"),
        ),
        Knob::CompressorAttack => (
            "Attack",
            "Bus compressor attack",
            "How fast compression starts.",
            Format::tenths("ms"),
        ),
        Knob::CompressorRelease => (
            "Release",
            "Bus compressor release",
            "How fast compression lets go.",
            Format::whole("ms"),
        ),
        Knob::MaximizerBoost => (
            "Boost",
            "Maximizer boost",
            "Louder, with peaks held below -1 dBFS. Each percent adds a quarter decibel.",
            Format::whole("%"),
        ),
        Knob::LiftLow => (
            "Low 90",
            "Lift EQ low",
            "A wide shelf of weight at 90 Hz.",
            Format::db(true),
        ),
        Knob::LiftAir => (
            "Air 12k",
            "Lift EQ air",
            "A wide shelf of air at 12 kHz.",
            Format::db(true),
        ),
        Knob::TapeDrive => (
            "Drive",
            "Tape drive",
            "How hard the mix hits the tape: saturation, a low-end bump, and a softer top.",
            Format::tenths(""),
        ),
        Knob::ImagingWidth => (
            "Width",
            "Stereo imaging width",
            "The side signal: below 100% narrows the image, above widens it.",
            Format::whole("%"),
        ),
        Knob::ImagingMonoBelow => (
            "Mono below",
            "Stereo imaging mono below",
            "The image is mono below this frequency.",
            Format::whole("Hz"),
        ),
        Knob::MultibandLow => (
            "Low",
            "Multiband low",
            "Below 200 Hz: holds the peaks and lifts the sound under them.",
            Format::whole("%"),
        ),
        Knob::MultibandMid => (
            "Mid",
            "Multiband mid",
            "From 200 Hz to 3 kHz: holds the peaks and lifts the sound under them.",
            Format::whole("%"),
        ),
        Knob::MultibandHigh => (
            "High",
            "Multiband high",
            "Above 3 kHz: holds the peaks and lifts the sound under them.",
            Format::whole("%"),
        ),
        Knob::LimiterGain => (
            "Gain",
            "Limiter gain",
            "Gain into the limiter.",
            Format::db(true),
        ),
        Knob::LimiterCeiling => (
            "Ceiling",
            "Limiter ceiling",
            "The highest true peak out, in dBTP.",
            Format::db(false),
        ),
        Knob::BoosterKey => (
            "Key",
            "Booster compressor key",
            "The track the compression follows, as it sounds after its fader. Own follows the mix.",
            Format::Key,
        ),
        Knob::CompressorKey => (
            "Key",
            "Bus compressor key",
            "The track the compressor listens to, as it sounds after its fader: the mix ducks \
             while that track sounds. Own listens to the mix.",
            Format::Key,
        ),
        Knob::MultibandKey => (
            "Key",
            "Multiband key",
            "A track whose bands each band also holds under its ceiling, so the mix gives way \
             band by band while that track sounds. Own: the mix alone.",
            Format::Key,
        ),
        Knob::LimiterKey => (
            "Key",
            "Limiter key",
            "A track the limiter also holds under the ceiling, so the mix gives way while that \
             track peaks. Own: the mix alone.",
            Format::Key,
        ),
    };
    (label.into(), name.into(), hint, format)
}

/// A setting as the decimal it was written as: `0.1_f32` is 0.1, not the
/// binary value just above it, so grid steps count true.
pub(crate) fn decimal(value: f32) -> f64 {
    (f64::from(value) * 1e6).round() / 1e6
}

/// The value on the knob's grid nearest to `value`, or one step on in the
/// direction it moved from `before`, so a fine step still moves one.
pub(super) fn on_grid(value: f64, before: Option<f64>, range: &KnobRange) -> f64 {
    let (low, high, step) = (decimal(range.low), decimal(range.high), decimal(range.step));
    let steps = (value - low) / step;
    let steps = match before {
        Some(before) if value > before => (steps - 1e-4).ceil(),
        Some(before) if value < before => (steps + 1e-4).floor(),
        _ => steps.round(),
    };
    let decimals = if step < 1.0 { 10f64.powi(2) } else { 1.0 };
    ((low + steps * step).clamp(low, high) * decimals).round() / decimals
}

/// A rotary with its value window and label. The value moves on the knob's
/// grid; the window takes typed values with or without the unit.
pub(crate) fn dial(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    value: &mut f64,
    spec: Dial<'_>,
    enabled: bool,
) {
    let [center, well, label] = positions;
    let range = spec.range;
    ui.painter().text(
        label,
        Align2::CENTER_CENTER,
        spec.label,
        FontId::proportional(11.5),
        if ui.visuals().dark_mode {
            roles().legend
        } else {
            roles().ink
        },
    );
    controls::well(ui, Rect::from_center_size(well, WELL));
    let (low, high) = (decimal(range.low), decimal(range.high));
    let hint = format!(
        "{}: {} to {}. {}",
        spec.name,
        spec.format.show(low),
        spec.format.show(high),
        if spec.hint.is_empty() {
            "Drag, or click the value to enter one."
        } else {
            spec.hint
        }
    );
    ui.add_enabled_ui(enabled, |ui| {
        let before = *value;
        let response = controls::rotary(
            ui,
            Rect::from_center_size(center, Vec2::splat(44.0)),
            value,
            controls::RotarySpec {
                name: spec.name,
                low,
                high,
                reset: decimal(range.default),
                step: decimal(range.step),
                decimals: if range.step < 1.0 { 1 } else { 0 },
                logarithmic: range.logarithmic,
                choices: spec.choices,
            },
        );
        // A drag, a reset, or a value set by assistive technology names a
        // place on the dial, which takes the nearest step. Only a key, wheel,
        // or increment step rounds on in its direction. A drag names its
        // place again every frame, so rounding it by direction would flip
        // between the two steps around it.
        let absolute = response.dragged()
            || response.double_clicked()
            || ui.input(|input| {
                input.num_accesskit_action_requests(response.id, egui::accesskit::Action::SetValue)
                    > 0
            });
        reveal(response).hover_hint(&hint);
        if spec.choices.is_empty() && *value != before {
            *value = on_grid(*value, (!absolute).then_some(before), &range);
        }
        ui.scope(|ui| {
            controls::value_style(ui);
            let typed = *value;
            let format = spec.format;
            // Leaving the window parses the text it shows. Unchanged text is
            // no edit, so a saved value between the shown digits stays.
            let entry = egui::DragValue::new(value)
                .range(low..=high)
                .speed(decimal(range.step))
                .update_while_editing(false)
                .custom_formatter(move |value, _| format.show(value))
                .custom_parser(move |text| {
                    if text.trim() == format.show(typed).trim() {
                        Some(typed)
                    } else {
                        format.read(text, high)
                    }
                });
            let response = ui.put(
                Rect::from_center_size(well, WELL + Vec2::new(-2.0, 4.0)),
                entry,
            );
            reveal(name_control(response, &format!("{} value", spec.name))).hover_hint(&hint);
            if *value != typed {
                *value = if spec.choices.is_empty() {
                    on_grid(*value, None, &range)
                } else if *value < (low + high) / 2.0 {
                    low
                } else {
                    high
                };
            }
        });
    });
}
