//! What sits on the song's timeline: clips with their fades, and named
//! sections.
use super::MAX_TICKS;
use serde::{Deserialize, Serialize};

/// The most characters in a section name.
pub const MAX_SECTION_NAME: usize = 48;

/// A named part of the song. It starts at `start_tick` and lasts until the
/// next section starts, or to the end of the song.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Section {
    pub start_tick: u32,
    pub name: String,
}

/// Why a section list cannot belong to a song, or `None` when it can: each
/// section starts on the timeline after the one before and has a short,
/// single-line name.
pub fn sections_error(sections: &[Section]) -> Option<String> {
    let mut previous = None;
    for section in sections {
        let name = section.name.trim();
        if name.is_empty()
            || name != section.name
            || name.chars().count() > MAX_SECTION_NAME
            || name.chars().any(char::is_control)
        {
            return Some(format!(
                "A section name must be 1 to {MAX_SECTION_NAME} characters on one line, \
                 without leading or trailing spaces."
            ));
        }
        if section.start_tick >= MAX_TICKS
            || previous.is_some_and(|tick| section.start_tick <= tick)
        {
            return Some(
                "Sections must start within the timeline, each after the one before.".into(),
            );
        }
        previous = Some(section.start_tick);
    }
    None
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Clip {
    pub id: u64,
    pub sample_id: u32,
    pub track: usize,
    pub start_tick: u32,
    pub length_ticks: u32,
    pub offset_frames: u32,
    pub gain: f32,
    /// The fade in from the clip's start and the fade out to the end of its
    /// sound: the clip's end, or earlier where the source audio ends.
    #[serde(default)]
    pub fades: Fades,
    /// A pitch change in cents that keeps the clip's timing.
    #[serde(default)]
    pub pitch_cents: i32,
    /// Play the clip backwards: the slice that starts at `offset_frames` and
    /// lasts the clip's length, reversed within the clip. Fades, pitch, and
    /// gain still apply in the clip's own time.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub reverse: bool,
}

/// The widest clip pitch change: two octaves either way.
pub const MAX_PITCH_CENTS: i32 = 2400;

/// The shape of a clip fade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FadeCurve {
    #[default]
    Linear,
    /// Constant power: two clips that cross with this curve keep their loudness.
    EqualPower,
    /// Even steps in decibels over 60 dB, like a natural decay.
    Exponential,
}

impl FadeCurve {
    pub const ALL: [Self; 3] = [Self::Linear, Self::EqualPower, Self::Exponential];

    pub fn label(self) -> &'static str {
        match self {
            Self::Linear => "Linear",
            Self::EqualPower => "Equal power",
            Self::Exponential => "Exponential",
        }
    }

    /// The gain at `progress` through a fade in, from 0 (silent) to 1 (full).
    /// A fade out reads the same curve backward.
    pub fn gain(self, progress: f32) -> f32 {
        let t = progress.clamp(0.0, 1.0);
        match self {
            Self::Linear => t,
            Self::EqualPower => (t * std::f32::consts::FRAC_PI_2).sin(),
            Self::Exponential => (1000.0_f32.powf(t) - 1.0) / 999.0,
        }
    }
}

/// One clip edge's fade. Zero ticks is no fade.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fade {
    pub ticks: u32,
    pub curve: FadeCurve,
}

/// A clip's two fades. Together they are no longer than the clip.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct Fades {
    #[serde(rename = "in")]
    pub fade_in: Fade,
    #[serde(rename = "out")]
    pub fade_out: Fade,
}

impl Fades {
    pub fn is_none(&self) -> bool {
        self.fade_in.ticks == 0 && self.fade_out.ticks == 0
    }
    /// Shorten the fades to fit a clip of `length_ticks`, the fade out first.
    pub fn fit(mut self, length_ticks: u32) -> Self {
        self.fade_in.ticks = self.fade_in.ticks.min(length_ticks);
        self.fade_out.ticks = self.fade_out.ticks.min(length_ticks - self.fade_in.ticks);
        self
    }
}
