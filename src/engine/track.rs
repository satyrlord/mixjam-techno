//! A track's mixer settings and the reading of saved tracks.
use super::TRACK_WIDTH;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct Track {
    /// Nonnegative amplitude. Polarity is independent, including at silence.
    pub gain: f32,
    pub polarity_inverted: bool,
    pub pan: f32,
    pub left_enabled: bool,
    pub right_enabled: bool,
    pub muted: bool,
    pub solo: bool,
    /// The share of the track sent to each FX bus, from 0 to 1, after the
    /// fader and pan.
    pub sends: [f32; crate::fx::BUSES],
    /// Lanes that move the volume, sends, filter, and pan over the song.
    pub automation: crate::automation::Automation,
    /// The channel EQ, between the clips and the automation.
    pub eq: crate::eq::ChannelEq,
    /// Adds harmonics to the track, after the EQ.
    pub drive: crate::drive::Drive,
    /// Evens out the track's level, after the drive.
    pub compressor: crate::compressor::Compressor,
    /// The stereo width in percent (`TRACK_WIDTH`), after the compressor
    /// and before the automation, fader, and pan.
    pub width: f32,
    /// Lowers the track while its key track sounds, after the level.
    pub ducker: crate::ducker::Ducker,
}
impl Default for Track {
    fn default() -> Self {
        Self {
            gain: 1.0,
            polarity_inverted: false,
            pan: 0.0,
            left_enabled: true,
            right_enabled: true,
            muted: false,
            solo: false,
            sends: [0.0; crate::fx::BUSES],
            automation: Default::default(),
            eq: Default::default(),
            drive: Default::default(),
            compressor: Default::default(),
            width: TRACK_WIDTH.default,
            ducker: Default::default(),
        }
    }
}

impl Track {
    pub fn effective_gain(&self) -> f32 {
        if self.polarity_inverted {
            -self.gain
        } else {
            self.gain
        }
    }

    /// Whether Mute and Solo let this track play. While any track is soloed,
    /// only soloed tracks play; Mute wins over Solo, as in the original.
    pub fn audible(&self, any_solo: bool) -> bool {
        !self.muted && (!any_solo || self.solo)
    }

    /// Preserve the effective amplitude of an older signed gain, including -0.0.
    pub fn set_legacy_gain(&mut self, signed: f32) {
        self.gain = signed.abs();
        self.polarity_inverted = signed.is_sign_negative();
    }

    /// Whether the width changes the track: both outputs on and a width
    /// other than 100%. A track that plays on one output has no side.
    pub fn widens(&self) -> bool {
        self.width != TRACK_WIDTH.default && self.left_enabled && self.right_enabled
    }
}

fn default_width() -> f32 {
    TRACK_WIDTH.default
}

impl<'de> Deserialize<'de> for Track {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct SavedTrack {
            gain: f32,
            #[serde(default, deserialize_with = "optional_bool")]
            polarity_inverted: Option<bool>,
            #[serde(default)]
            pan: f32,
            left_enabled: bool,
            right_enabled: bool,
            // Projects saved before Mute and Solo existed have neither.
            #[serde(default)]
            muted: bool,
            #[serde(default)]
            solo: bool,
            // Projects saved before the FX buses existed send nothing.
            #[serde(default)]
            sends: [f32; crate::fx::BUSES],
            #[serde(default)]
            automation: crate::automation::Automation,
            // Projects saved before the channel EQ existed have it off.
            #[serde(default)]
            eq: crate::eq::ChannelEq,
            // Projects saved before the drive existed have it off.
            #[serde(default)]
            drive: crate::drive::Drive,
            // Projects saved before the compressor and width existed have
            // the compressor off and the track as wide as its clips.
            #[serde(default)]
            compressor: crate::compressor::Compressor,
            #[serde(default = "default_width")]
            width: f32,
            // Projects saved before the ducker existed have it off.
            #[serde(default)]
            ducker: crate::ducker::Ducker,
        }
        let saved = SavedTrack::deserialize(deserializer)?;
        let mut track = Self {
            gain: saved.gain,
            polarity_inverted: saved.polarity_inverted.unwrap_or(false),
            pan: saved.pan,
            left_enabled: saved.left_enabled,
            right_enabled: saved.right_enabled,
            muted: saved.muted,
            solo: saved.solo,
            sends: saved.sends,
            automation: saved.automation,
            eq: saved.eq,
            drive: saved.drive,
            compressor: saved.compressor,
            width: saved.width,
            ducker: saved.ducker,
        };
        if saved.polarity_inverted.is_none() {
            if !saved.gain.is_finite() || !(-2.0..=4.0).contains(&saved.gain) {
                return Err(serde::de::Error::custom(
                    "Legacy track gain must be from -2 to 4.",
                ));
            }
            track.set_legacy_gain(saved.gain);
        }
        Ok(track)
    }
}

/// A field that may be absent but, when present, must be a boolean, not null.
pub(crate) fn optional_bool<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> Result<Option<bool>, D::Error> {
    bool::deserialize(deserializer).map(Some)
}
