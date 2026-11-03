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

/// How a clip reads its source when it plays off the source's own tempo or
/// sample rate, or under a Pitch lane.
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
    pub drive: Oversampling,
    pub master: Oversampling,
    pub samples: SampleQuality,
}

impl PlaybackQuality {
    /// Every export and song-tool render explicitly uses full quality.
    pub const FULL: Self = Self {
        drive: Oversampling::FourX,
        master: Oversampling::FourX,
        samples: SampleQuality::High,
    };
}
