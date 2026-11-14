//! File-format readers for the T1 profile: songs, sample
//! catalogs, compressed samples, and WAV admission.
/// The rate of every native sample: 16-bit mono at 44.1 kHz. This is a
/// source format, not the mix rate, which the engine owns.
pub const NATIVE_SAMPLE_RATE: u32 = 44_100;

pub mod editing;
pub mod imports;
pub mod library;
pub mod mix;
pub mod packed;
pub mod playback;
pub mod timeline;
pub mod wav;
