mod editor;
mod load;
mod project;
mod samples;
mod timeline;
mod timing;
mod track;
mod validate;

pub use editor::Editor;
pub(crate) use editor::ensure_space;
pub use load::normalize_import;
pub use project::Project;
pub use samples::{
    ImportNotice, ImportedSample, ImportedSamples, MissingReference, MissingSample, SampleConflict,
    rescale_trim, validate_import_metadata,
};
pub use timeline::{
    Clip, Fade, FadeCurve, Fades, MAX_PITCH_CENTS, MAX_SECTION_NAME, Section, sections_error,
};
pub use timing::{
    TICKS_PER_BEAT, frames_per_tick_at, nearest_supported_bpm, playback_bpm, tick_seconds,
};
pub(crate) use track::optional_bool;
pub use track::{Silence, Track, TrackMix};

pub const PROJECT_SCHEMA_VERSION: u32 = 4;
pub const TRACK_COUNT: usize = 64;
/// The most tracks that the duckers, the master pedals, and the FX buses
/// may use as keys together. Each key is mixed on its own while the song
/// mixes.
pub const MAX_KEYS: usize = 8;
pub const SUPPORTED_BPMS: [f64; 7] = [120.0, 125.0, 130.0, 135.0, 140.0, 160.0, 180.0];
pub const TICKS_PER_BAR: u32 = 256;
pub const SOURCE_BPM: f64 = 140.0;
/// The tempos, in BPM, that a sound's source tempo, a time stretch, a
/// recording, and an effect render take.
pub const TEMPO_RANGE: std::ops::RangeInclusive<f64> = 7.5..=1200.0;
/// A new song's title until it is saved under a file name.
pub const DEFAULT_TITLE: &str = "Untitled";
pub const MAX_TICKS: u32 = 4096 * TICKS_PER_BAR;
/// A guard against malformed songs. It is above the 262,144 one-bar clips
/// that the 64-track timeline holds.
pub const MAX_CLIPS: usize = 1_000_000;
pub const MAX_MIXER_GAIN_DB: f32 = 12.0;
/// A track's stereo width in percent: 0 is mono, 100 leaves the track as
/// it is, and 200 doubles its side.
pub const TRACK_WIDTH: crate::master::KnobRange = crate::master::range(0.0, 200.0, 100.0, 1.0);

/// The pitch shifts that a clip and the sample editor take, in cents.
pub const PITCH_RANGE: std::ops::RangeInclusive<i32> = -MAX_PITCH_CENTS..=MAX_PITCH_CENTS;

/// Check that `cents` is in [`PITCH_RANGE`]. The error names `subject`, as
/// in "Clip pitch must be from -24 to 24 semitones."
pub fn check_pitch(cents: i32, subject: impl std::fmt::Display) -> Result<(), String> {
    if PITCH_RANGE.contains(&cents) {
        Ok(())
    } else {
        Err(format!(
            "{subject} must be from {} to {} semitones.",
            PITCH_RANGE.start() / 100,
            PITCH_RANGE.end() / 100
        ))
    }
}

/// Convert a nonnegative amplitude multiplier to decibels. Zero is silence.
pub fn amplitude_to_db(amplitude: f32) -> f32 {
    (20.0 * f64::from(amplitude).log10()) as f32
}

/// Convert decibels to a nonnegative amplitude multiplier. Negative infinity is silence.
pub fn db_to_amplitude(db: f32) -> f32 {
    10.0_f64.powf(f64::from(db) / 20.0) as f32
}

/// The loudest clip gain as amplitude, about +6.02 dB. A clip gain is from 0
/// to this value.
pub const MAX_CLIP_GAIN: f32 = 2.0;
/// A clip gain in dB at or below this level is silence.
pub const CLIP_SILENCE_DB: f32 = -60.0;

/// The loudest clip gain in dB, about +6.02.
pub fn clip_gain_max_db() -> f32 {
    amplitude_to_db(MAX_CLIP_GAIN)
}

/// A clip's gain in dB, from `CLIP_SILENCE_DB` for silence to the maximum.
pub fn clip_gain_db(gain: f32) -> f32 {
    if gain > 0.0 {
        amplitude_to_db(gain).max(CLIP_SILENCE_DB)
    } else {
        CLIP_SILENCE_DB
    }
}

/// A clip's gain amplitude from dB. `CLIP_SILENCE_DB` and below are silence,
/// and the maximum and above are `MAX_CLIP_GAIN`.
pub fn clip_gain_from_db(db: f32) -> f32 {
    if db <= CLIP_SILENCE_DB {
        0.0
    } else if db >= clip_gain_max_db() {
        MAX_CLIP_GAIN
    } else {
        db_to_amplitude(db)
    }
}

#[cfg(test)]
mod tests;
