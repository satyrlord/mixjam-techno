//! The parameter lettering of the FX returns units: which dials a unit
//! shows, and each dial's printed label, name, hint, and format.
use super::master::Format;
use super::master::{Dial, compact_dial, dial};
use super::*;
use mixjam_techno::application::KnobRange;
use mixjam_techno::application::fx::{
    DelayParam, DistortionParam, FxBus, Module, Range, ReverbParam,
};

/// Hero macro dials displayed on each compact Layer 1 card (`<= 4` knobs).
pub(super) fn macro_slots(module: &Module) -> &'static [Slot] {
    match module {
        Module::Empty => &[Slot::Return],
        Module::EchoformDelay(_) => &[
            Slot::Delay(DelayParam::TimeL),
            Slot::Delay(DelayParam::Feedback),
            Slot::Delay(DelayParam::HighCut),
            Slot::Return,
        ],
        Module::AetherformReverb(_) => &[
            Slot::Reverb(ReverbParam::Decay),
            Slot::Reverb(ReverbParam::Size),
            Slot::Reverb(ReverbParam::HighCut),
            Slot::Return,
        ],
        Module::EmberformDrive(_) => &[
            Slot::Drive(DistortionParam::Drive),
            Slot::Drive(DistortionParam::Asymmetry),
            Slot::Drive(DistortionParam::HighCut),
            Slot::Return,
        ],
    }
}

/// Shared dial painter and setter for any `Slot` (used by both Layer 1 and
/// Deep Edit). `compact` is the value well's width of a compact dial; none
/// draws the full dial.
pub(super) fn dial_slot(
    ui: &mut egui::Ui,
    positions: [Pos2; 3],
    prefix: &str,
    bus: &mut FxBus,
    slot: &Slot,
    compact: Option<f32>,
) {
    let draw = Draw {
        compact,
        ui,
        positions,
        prefix,
    };
    match (slot, &mut bus.module) {
        (Slot::Return, _) => return_dial(draw, &mut bus.return_level),
        (Slot::Delay(param), Module::EchoformDelay(delay)) => {
            let shown = (param.range(), param.get(delay));
            let enabled = delay_dial_enabled(*param, delay);
            if let Some(value) = draw_face_dial(draw, delay_face(*param), shown, enabled) {
                param.set(delay, value);
            }
        }
        (Slot::Reverb(param), Module::AetherformReverb(reverb)) => {
            let shown = (param.range(), param.get(reverb));
            let enabled = reverb_dial_enabled(*param, reverb);
            if let Some(value) = draw_face_dial(draw, reverb_face(*param), shown, enabled) {
                param.set(reverb, value);
            }
        }
        (Slot::Drive(param), Module::EmberformDrive(drive)) => {
            let shown = (param.range(), param.get(drive));
            let enabled = drive_dial_enabled(*param, drive);
            if let Some(value) = draw_face_dial(draw, drive_face(*param), shown, enabled) {
                param.set(drive, value);
            }
        }
        // A slot of another effect than the bus's has no dial.
        _ => {}
    }
}

/// Whether the delay's dial for `param` acts: the times only in Free mode,
/// and the right time only while it is not linked to the left.
fn delay_dial_enabled(param: DelayParam, delay: &mixjam_techno::application::fx::Echoform) -> bool {
    let free = delay.mode == mixjam_techno::application::fx::TimeMode::Free;
    match param {
        DelayParam::TimeL => free,
        DelayParam::TimeR => free && !delay.link_times,
        _ => true,
    }
}

/// Whether the reverb's dial for `param` acts: the shimmer amount only
/// while Shimmer is on.
fn reverb_dial_enabled(
    param: ReverbParam,
    reverb: &mixjam_techno::application::fx::Aetherform,
) -> bool {
    param != ReverbParam::ShimmerAmount || reverb.shimmer_enabled
}

/// Whether the drive's dial for `param` acts: the crusher's bits and rate
/// only with the Bitcrush type.
fn drive_dial_enabled(
    param: DistortionParam,
    drive: &mixjam_techno::application::fx::Emberform,
) -> bool {
    !matches!(param, DistortionParam::Bits | DistortionParam::CrushRate)
        || drive.shape == mixjam_techno::application::fx::DistortionShape::Bitcrush
}

/// Where and how a slot's dial is drawn.
struct Draw<'a, 'b> {
    /// The compact value well's width; none draws the full dial.
    compact: Option<f32>,
    ui: &'a mut egui::Ui,
    positions: [Pos2; 3],
    /// The bus's name, which starts each dial's name.
    prefix: &'b str,
}

/// Draws `spec` as `draw` says, compact or full.
fn draw_spec(draw: Draw<'_, '_>, value: &mut f64, spec: Dial<'_>, enabled: bool) {
    let Draw {
        compact,
        ui,
        positions,
        ..
    } = draw;
    match compact {
        Some(well) => compact_dial(ui, positions, value, spec, enabled, well),
        None => dial(ui, positions, value, spec, enabled),
    }
}

/// The bus's return level, in whole percent.
fn return_dial(draw: Draw<'_, '_>, level: &mut f32) {
    let mut value = f64::from((*level * 100.0).round());
    let before = value;
    let prefix = draw.prefix;
    draw_spec(
        draw,
        &mut value,
        Dial {
            name: &format!("{prefix} return level"),
            label: "Return",
            hint: "The level of the effect's sound in the mix.",
            range: KnobRange {
                low: 0.0,
                high: 100.0,
                default: 100.0,
                step: 1.0,
                logarithmic: false,
            },
            format: Format::whole("%"),
            choices: &[],
        },
        true,
    );
    if value != before {
        *level = (value / 100.0) as f32;
    }
}

fn draw_face_dial(
    draw: Draw<'_, '_>,
    face: Face,
    (range, current): (Range, f32),
    enabled: bool,
) -> Option<f32> {
    let mut value = f64::from(current);
    let before = value;
    let prefix = draw.prefix;
    draw_spec(
        draw,
        &mut value,
        Dial {
            name: &format!("{prefix} {}", face.name),
            label: face.label,
            hint: face.hint,
            range: knob(range),
            format: face.format,
            choices: &[],
        },
        enabled,
    );
    (value != before).then_some(value as f32)
}

/// What one dial prints: its label, the name after the bus's, its hint, and
/// the format of its value.
pub(super) struct Face {
    pub(super) label: &'static str,
    pub(super) name: &'static str,
    pub(super) hint: &'static str,
    pub(super) format: Format,
}

/// The dials a unit shows: the return level first, then the effect's own.
pub(super) enum Slot {
    Return,
    Delay(DelayParam),
    Reverb(ReverbParam),
    Drive(DistortionParam),
}

#[cfg(test)]
pub(super) fn slots(module: &Module) -> Vec<Slot> {
    match module {
        Module::Empty => Vec::new(),
        Module::EchoformDelay(_) => std::iter::once(Slot::Return)
            .chain(DelayParam::ALL.into_iter().map(Slot::Delay))
            .collect(),
        Module::AetherformReverb(_) => std::iter::once(Slot::Return)
            .chain(
                [
                    ReverbParam::PreDelay,
                    ReverbParam::Decay,
                    ReverbParam::Size,
                    ReverbParam::BassMult,
                    ReverbParam::TrebleMult,
                    ReverbParam::LateBalance,
                    ReverbParam::Diffusion,
                    ReverbParam::Density,
                    ReverbParam::LowCut,
                    ReverbParam::HighCut,
                    ReverbParam::Width,
                    ReverbParam::Drive,
                    ReverbParam::ModRate,
                    ReverbParam::ModDepth,
                    ReverbParam::ShimmerAmount,
                    ReverbParam::DuckThreshold,
                    ReverbParam::DuckAmount,
                    ReverbParam::DuckRelease,
                    ReverbParam::DuckKey,
                    ReverbParam::Output,
                ]
                .into_iter()
                .map(Slot::Reverb),
            )
            .collect(),
        Module::EmberformDrive(_) => std::iter::once(Slot::Return)
            .chain(DistortionParam::ALL.into_iter().map(Slot::Drive))
            .collect(),
    }
}

pub(super) fn knob(range: Range) -> KnobRange {
    KnobRange {
        low: range.low,
        high: range.high,
        default: range.default,
        step: range.step,
        logarithmic: range.log,
    }
}

const fn in_unit(unit: &'static str, decimals: usize) -> Format {
    Format::Unit {
        unit,
        decimals,
        signed: false,
    }
}

/// The lettering of the delay's dial for `param`.
pub(super) fn delay_face(param: DelayParam) -> Face {
    match param {
        DelayParam::TimeL
        | DelayParam::TimeR
        | DelayParam::Feedback
        | DelayParam::Width
        | DelayParam::LowCut
        | DelayParam::HighCut
        | DelayParam::Output => delay_time_face(param),
        DelayParam::FilterResonance
        | DelayParam::Smear
        | DelayParam::ModRate
        | DelayParam::ModDepth
        | DelayParam::Drive => delay_colour_face(param),
        DelayParam::DuckThreshold
        | DelayParam::DuckAmount
        | DelayParam::DuckRelease
        | DelayParam::DuckKey => delay_duck_face(param),
    }
}

/// The lettering of the delay's time, feedback, filter, and output dials.
fn delay_time_face(param: DelayParam) -> Face {
    match param {
        DelayParam::TimeL => Face {
            label: "Time L",
            name: "left time",
            hint: "The left delay in Free mode.",
            format: in_unit("ms", 0),
        },
        DelayParam::TimeR => Face {
            label: "Time R",
            name: "right time",
            hint: "The right delay in Free mode.",
            format: in_unit("ms", 0),
        },
        DelayParam::Feedback => Face {
            label: "Feedback",
            name: "feedback",
            hint: "How much of each repeat returns. Above 100% the loop's soft limit holds it.",
            format: Format::whole("%"),
        },
        DelayParam::Width => Face {
            label: "Width",
            name: "width",
            hint: "0% is mono, 100% unchanged, 200% doubles the sides.",
            format: Format::whole("%"),
        },
        DelayParam::LowCut => Face {
            label: "Low cut",
            name: "low cut",
            hint: "Each repeat loses more below this.",
            format: Format::Hertz,
        },
        DelayParam::HighCut => Face {
            label: "High cut",
            name: "high cut",
            hint: "Each repeat loses more above this.",
            format: Format::Hertz,
        },
        DelayParam::Output => Face {
            label: "Output",
            name: "output",
            hint: "Output level trim after the delay and its filters.",
            format: Format::db(true),
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the delay's resonance, smear, modulation, and drive dials.
fn delay_colour_face(param: DelayParam) -> Face {
    match param {
        DelayParam::FilterResonance => Face {
            label: "Res",
            name: "filter resonance",
            hint: "Resonance around the delay filter cutoff.",
            format: Format::whole("%"),
        },
        DelayParam::Smear => Face {
            label: "Smear",
            name: "smear",
            hint: "Diffuses each repeat into a softer bloom.",
            format: Format::whole("%"),
        },
        DelayParam::ModRate => Face {
            label: "Rate",
            name: "modulation rate",
            hint: "How fast the delay modulation LFO sweeps the delay time.",
            format: in_unit("Hz", 2),
        },
        DelayParam::ModDepth => Face {
            label: "Depth",
            name: "modulation depth",
            hint: "The peak change of the delay time.",
            format: Format::tenths("ms"),
        },
        DelayParam::Drive => Face {
            label: "Drive",
            name: "drive",
            hint: "Saturates the sound entering the delay.",
            format: Format::whole("%"),
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the delay's ducker dials.
fn delay_duck_face(param: DelayParam) -> Face {
    match param {
        DelayParam::DuckThreshold => Face {
            label: "Thresh",
            name: "duck threshold",
            hint: "Input level above which ducking begins.",
            format: Format::db(true),
        },
        DelayParam::DuckAmount => Face {
            label: "Duck",
            name: "duck amount",
            hint: "Lowers the repeats while the input plays.",
            format: Format::whole("%"),
        },
        DelayParam::DuckRelease => Face {
            label: "Release",
            name: "duck release",
            hint: "How fast the repeats return.",
            format: Format::whole("ms"),
        },
        DelayParam::DuckKey => Face {
            label: "Key",
            name: "duck key",
            hint: "The track that lowers the repeats, as it sounds after its fader. Own: the input.",
            format: Format::Key,
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the reverb's dial for `param`.
pub(super) fn reverb_face(param: ReverbParam) -> Face {
    match param {
        ReverbParam::PreDelay
        | ReverbParam::Decay
        | ReverbParam::Size
        | ReverbParam::LateBalance
        | ReverbParam::Diffusion
        | ReverbParam::Density => reverb_space_face(param),
        ReverbParam::BassMult
        | ReverbParam::TrebleMult
        | ReverbParam::LowCut
        | ReverbParam::HighCut
        | ReverbParam::Width
        | ReverbParam::Drive => reverb_tone_face(param),
        ReverbParam::ModRate
        | ReverbParam::ModDepth
        | ReverbParam::ShimmerAmount
        | ReverbParam::Output => reverb_motion_face(param),
        ReverbParam::DuckThreshold
        | ReverbParam::DuckAmount
        | ReverbParam::DuckRelease
        | ReverbParam::DuckKey => reverb_duck_face(param),
    }
}

/// The lettering of the reverb's space dials.
fn reverb_space_face(param: ReverbParam) -> Face {
    match param {
        ReverbParam::PreDelay => Face {
            label: "Pre-delay",
            name: "pre-delay",
            hint: "The gap before the space answers.",
            format: Format::whole("ms"),
        },
        ReverbParam::Decay => Face {
            label: "Decay",
            name: "decay",
            hint: "The time the tail takes to fall by 60 dB.",
            format: in_unit("s", 2),
        },
        ReverbParam::Size => Face {
            label: "Size",
            name: "size",
            hint: "The size of the space. It does not change the decay.",
            format: Format::whole("%"),
        },
        ReverbParam::LateBalance => Face {
            label: "Late",
            name: "late balance",
            hint: "0% early reflections only, 100% the tail only.",
            format: Format::whole("%"),
        },
        ReverbParam::Diffusion => Face {
            label: "Diffusion",
            name: "diffusion",
            hint: "How fast reflections blur together.",
            format: Format::whole("%"),
        },
        ReverbParam::Density => Face {
            label: "Density",
            name: "density",
            hint: "How many reflections, and how full the tail.",
            format: Format::whole("%"),
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the reverb's damping, tone, and drive dials.
fn reverb_tone_face(param: ReverbParam) -> Face {
    match param {
        ReverbParam::BassMult => Face {
            label: "Bass",
            name: "bass multiplier",
            hint: "Scales low-frequency decay relative to the main decay time.",
            format: in_unit("x", 2),
        },
        ReverbParam::TrebleMult => Face {
            label: "Treble",
            name: "treble multiplier",
            hint: "Scales high-frequency decay relative to the main decay time.",
            format: in_unit("x", 2),
        },
        ReverbParam::LowCut => Face {
            label: "Low cut",
            name: "low cut",
            hint: "The tail loses more below this as it decays.",
            format: Format::Hertz,
        },
        ReverbParam::HighCut => Face {
            label: "High cut",
            name: "high cut",
            hint: "The tail loses more above this as it decays.",
            format: Format::Hertz,
        },
        ReverbParam::Width => Face {
            label: "Width",
            name: "width",
            hint: "0% is mono, 100% unchanged, 200% doubles the sides.",
            format: Format::whole("%"),
        },
        ReverbParam::Drive => Face {
            label: "Drive",
            name: "drive",
            hint: "Saturates the sound entering the reverb.",
            format: Format::whole("%"),
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the reverb's modulation, shimmer, and output dials.
fn reverb_motion_face(param: ReverbParam) -> Face {
    match param {
        ReverbParam::ModRate => Face {
            label: "Rate",
            name: "modulation rate",
            hint: "How fast pitch modulation sweeps inside the reverb tank.",
            format: in_unit("Hz", 2),
        },
        ReverbParam::ModDepth => Face {
            label: "Depth",
            name: "modulation depth",
            hint: "How far modulation bends pitch inside the reverb tank to smooth metallic ringing.",
            format: Format::whole("%"),
        },
        ReverbParam::ShimmerAmount => Face {
            label: "Shimmer",
            name: "shimmer amount",
            hint: "How much of the tail returns pitched up. It acts while Shimmer is on.",
            format: Format::whole("%"),
        },
        ReverbParam::Output => Face {
            label: "Output",
            name: "output",
            hint: "Output level trim after the reverb and its filters.",
            format: Format::db(true),
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the reverb's ducker dials.
fn reverb_duck_face(param: ReverbParam) -> Face {
    match param {
        ReverbParam::DuckThreshold => Face {
            label: "Thresh",
            name: "duck threshold",
            hint: "Input level above which ducking begins.",
            format: Format::db(true),
        },
        ReverbParam::DuckAmount => Face {
            label: "Duck",
            name: "duck amount",
            hint: "Lowers the tail while the input plays.",
            format: Format::whole("%"),
        },
        ReverbParam::DuckRelease => Face {
            label: "Release",
            name: "duck release",
            hint: "How fast the tail returns.",
            format: Format::whole("ms"),
        },
        ReverbParam::DuckKey => Face {
            label: "Key",
            name: "duck key",
            hint: "The track that lowers the tail, as it sounds after its fader. Own: the input.",
            format: Format::Key,
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the drive's dial for `param`.
pub(super) fn drive_face(param: DistortionParam) -> Face {
    match param {
        DistortionParam::Drive
        | DistortionParam::Asymmetry
        | DistortionParam::Bits
        | DistortionParam::CrushRate => drive_curve_face(param),
        DistortionParam::LowCut
        | DistortionParam::HighCut
        | DistortionParam::Width
        | DistortionParam::Output => drive_tone_face(param),
        DistortionParam::DuckThreshold
        | DistortionParam::DuckAmount
        | DistortionParam::DuckRelease
        | DistortionParam::DuckKey => drive_duck_face(param),
    }
}

/// The lettering of the drive's curve and crusher dials.
fn drive_curve_face(param: DistortionParam) -> Face {
    match param {
        DistortionParam::Drive => Face {
            label: "Drive",
            name: "drive",
            hint: "How hard the sound goes into the curve. The level stays; the tone changes.",
            format: Format::tenths("dB"),
        },
        DistortionParam::Asymmetry => Face {
            label: "Asym",
            name: "asymmetry",
            hint: "Bends one half of the wave more than the other: more even harmonics.",
            format: Format::whole("%"),
        },
        DistortionParam::Bits => Face {
            label: "Bits",
            name: "bits",
            hint: "The crusher's bit depth. It acts with the Bitcrush type.",
            format: Format::whole(""),
        },
        DistortionParam::CrushRate => Face {
            label: "Rate",
            name: "crush rate",
            hint: "The crusher's sample rate: lower folds more aliasing in. It acts with the Bitcrush \
             type.",
            format: Format::Hertz,
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the drive's filter, width, and output dials.
fn drive_tone_face(param: DistortionParam) -> Face {
    match param {
        DistortionParam::LowCut => Face {
            label: "Low cut",
            name: "low cut",
            hint: "Takes the lows out before the curve, so they stay clean in the dry sound.",
            format: Format::Hertz,
        },
        DistortionParam::HighCut => Face {
            label: "High cut",
            name: "high cut",
            hint: "Softens the fizz above this after the curve.",
            format: Format::Hertz,
        },
        DistortionParam::Width => Face {
            label: "Width",
            name: "width",
            hint: "0% is mono, 100% unchanged, 200% doubles the sides.",
            format: Format::whole("%"),
        },
        DistortionParam::Output => Face {
            label: "Output",
            name: "output",
            hint: "Output level trim after the drive stage.",
            format: Format::db(true),
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

/// The lettering of the drive's ducker dials.
fn drive_duck_face(param: DistortionParam) -> Face {
    match param {
        DistortionParam::DuckThreshold => Face {
            label: "Thresh",
            name: "duck threshold",
            hint: "Input level above which ducking begins.",
            format: Format::db(true),
        },
        DistortionParam::DuckAmount => Face {
            label: "Duck",
            name: "duck amount",
            hint: "Lowers the distortion while the input plays.",
            format: Format::whole("%"),
        },
        DistortionParam::DuckRelease => Face {
            label: "Release",
            name: "duck release",
            hint: "How fast the distortion returns.",
            format: Format::whole("ms"),
        },
        DistortionParam::DuckKey => Face {
            label: "Key",
            name: "duck key",
            hint: "The track that lowers the distortion, as it sounds after its fader. Own: the input.",
            format: Format::Key,
        },
        _ => unreachable!("{param:?} is not in this group"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mixjam_techno::application::fx::{Aetherform, Echoform, Emberform};

    #[test]
    fn every_unit_fits_its_macro_dials_on_a_compact_card() {
        for module in [
            Module::Empty,
            Module::EchoformDelay(Echoform::default()),
            Module::AetherformReverb(Aetherform::default()),
            Module::EmberformDrive(Emberform::default()),
        ] {
            let macros = macro_slots(&module).len();
            assert!((1..=4).contains(&macros));
        }
        assert_eq!(
            slots(&Module::AetherformReverb(Aetherform::default())).len(),
            21
        );
    }
}
