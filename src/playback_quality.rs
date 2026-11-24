//! Runtime playback quality. It is not part of a song or a master pedalboard.
use serde::{Deserialize, Serialize};

/// Oversampling of saturation curves during playback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum Oversampling {
    #[default]
    #[serde(rename = "4x")]
    FourX,
    #[serde(rename = "off")]
    Off,
}

/// Oversampling of track Drive during playback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum DriveOversampling {
    #[default]
    #[serde(rename = "2x", alias = "4x")]
    TwoX,
    #[serde(rename = "off")]
    Off,
}

/// How sources at a different rate or tempo play during playback.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub enum SampleQuality {
    /// A band-limited read: a windowed-sinc filter that reduces aliasing.
    /// See `audio::resample` for its transition-band limits.
    #[default]
    #[serde(rename = "high")]
    High,
    /// Straight-line interpolation between two source frames: less CPU, but
    /// it aliases off the source's tempo and rate.
    #[serde(rename = "low")]
    Low,
}

/// The independent quality of the track Drives, the master saturation
/// pedals, and the clips' sample reads.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaybackQuality {
    pub drive: DriveOversampling,
    pub master: Oversampling,
    pub samples: SampleQuality,
}

impl PlaybackQuality {
    /// Every export and song-tool render explicitly uses full quality.
    pub const FULL: Self = Self {
        drive: DriveOversampling::TwoX,
        master: Oversampling::FourX,
        samples: SampleQuality::High,
    };
}
