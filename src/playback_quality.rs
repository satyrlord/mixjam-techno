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

/// The independent quality of the track Drives and master saturation pedals.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaybackQuality {
    pub drive: Oversampling,
    pub master: Oversampling,
}

impl PlaybackQuality {
    /// Every export and song-tool render explicitly uses full quality.
    pub const FULL: Self = Self {
        drive: Oversampling::FourX,
        master: Oversampling::FourX,
    };
}
