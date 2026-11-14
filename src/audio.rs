mod device_mixer;
mod export;
pub(crate) mod flight;
mod frames;
mod handoff;
mod latest;
mod mix;
mod output;
mod pcm;
mod player;
mod prepare;
mod resample;
mod source;
mod track;

pub use export::Stems;
pub use frames::{Frames, RenderState};
pub use mix::Mix;
pub use output::OutputConfiguration;
pub use pcm::{Pcm, read_wav};
pub use player::Player;
pub use prepare::{prepare_project, prepare_project_with_quality, render_project};
pub use source::priority_failure;

#[cfg(test)]
pub(crate) use frames::track_levels;
pub(crate) use latest::Latest;
pub(crate) use mix::hold_to_full_scale;
pub(crate) use pcm::{MAX_WAV_SAMPLES, float_to_i16, hide_stray_end, visit_wav_samples};
pub(crate) use prepare::prepare_project_reusing_with_quality;
#[cfg(test)]
pub(crate) use prepare::{mix_with_samples, prepare_project_reusing, render_with_samples};
#[cfg(test)]
pub(crate) use source::play_offline;

#[cfg(test)]
mod device_chain_tests;
#[cfg(test)]
mod realtime_bench;
#[cfg(test)]
mod tests;
