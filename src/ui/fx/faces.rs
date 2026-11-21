//! The parameter lettering of the FX returns units: which dials a unit
//! shows, and each dial's printed label, name, hint, and format.
use super::master::Format;
use super::*;
use mixjam_techno::application::KnobRange;
use mixjam_techno::application::fx::{DelayParam, DistortionParam, Module, Range, ReverbParam};

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

/// Dials in a row: two rows hold them all.
pub(super) fn columns(module: &Module) -> usize {
    slots(module).len().div_ceil(2)
}

pub(super) fn unit_width(module: &Module) -> f32 {
    (columns(module) as f32 * CELL + 2.0 * PADDING).max(320.0)
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
            format: Format::whole("Hz"),
        },
        DelayParam::HighCut => Face {
            label: "High cut",
            name: "high cut",
            hint: "Each repeat loses more above this.",
            format: Format::whole("Hz"),
        },
        DelayParam::ModRate => Face {
            label: "Rate",
            name: "modulation rate",
            hint: "",
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
        DelayParam::Output => Face {
            label: "Output",
            name: "output",
            hint: "",
            format: Format::db(true),
        },
    }
}

/// The lettering of the reverb's dial for `param`.
pub(super) fn reverb_face(param: ReverbParam) -> Face {
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
        ReverbParam::LowCut => Face {
            label: "Low cut",
            name: "low cut",
            hint: "The tail loses more below this as it decays.",
            format: Format::whole("Hz"),
        },
        ReverbParam::HighCut => Face {
            label: "High cut",
            name: "high cut",
            hint: "The tail loses more above this as it decays.",
            format: Format::whole("Hz"),
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
        ReverbParam::ModRate => Face {
            label: "Rate",
            name: "modulation rate",
            hint: "",
            format: in_unit("Hz", 2),
        },
        ReverbParam::ModDepth => Face {
            label: "Depth",
            name: "modulation depth",
            hint: "",
            format: Format::whole("%"),
        },
        ReverbParam::ShimmerAmount => Face {
            label: "Shimmer",
            name: "shimmer amount",
            hint: "How much of the tail returns pitched up. It acts while Shimmer is on.",
            format: Format::whole("%"),
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
        ReverbParam::Output => Face {
            label: "Output",
            name: "output",
            hint: "",
            format: Format::db(true),
        },
    }
}

/// The lettering of the drive's dial for `param`.
pub(super) fn drive_face(param: DistortionParam) -> Face {
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
        DistortionParam::LowCut => Face {
            label: "Low cut",
            name: "low cut",
            hint: "Takes the lows out before the curve, so they stay clean in the dry sound.",
            format: Format::whole("Hz"),
        },
        DistortionParam::HighCut => Face {
            label: "High cut",
            name: "high cut",
            hint: "Softens the fizz above this after the curve.",
            format: Format::whole("Hz"),
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
            format: Format::whole("Hz"),
        },
        DistortionParam::Width => Face {
            label: "Width",
            name: "width",
            hint: "0% is mono, 100% unchanged, 200% doubles the sides.",
            format: Format::whole("%"),
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
        DistortionParam::Output => Face {
            label: "Output",
            name: "output",
            hint: "",
            format: Format::db(true),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mixjam_techno::application::fx::{Aetherform, Echoform, Emberform};

    #[test]
    fn every_unit_fits_its_dials_in_two_rows() {
        for module in [
            Module::Empty,
            Module::EchoformDelay(Echoform::default()),
            Module::AetherformReverb(Aetherform::default()),
            Module::EmberformDrive(Emberform::default()),
        ] {
            let count = slots(&module).len();
            assert!(count <= 2 * columns(&module));
            assert!(columns(&module) as f32 * CELL + 2.0 * PADDING <= unit_width(&module));
        }
        assert_eq!(
            slots(&Module::AetherformReverb(Aetherform::default())).len(),
            18
        );
    }
}
