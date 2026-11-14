//! Song time and tempo: the end of the timeline, the span of a section,
//! frames per tick, and the tempo at which a sound plays.
use super::{MAX_TICKS, Project, SUPPORTED_BPMS, TICKS_PER_BAR};

impl Project {
    pub fn end_tick(&self) -> u32 {
        self.clips
            .iter()
            .map(|c| c.start_tick.saturating_add(c.length_ticks))
            .max()
            .unwrap_or(TICKS_PER_BAR * 4)
            .max(self.loop_region.map_or(0, |[_, b]| b))
    }

    /// The span of the section that plays at `tick`, as `[start, end)`. Before
    /// the first section, the span is the lead-in up to it. `None` without
    /// sections.
    pub fn section_span(&self, tick: u32) -> Option<[u32; 2]> {
        let first = self.sections.first()?;
        let next = self.sections.iter().position(|s| s.start_tick > tick);
        let (start, end) = match next {
            Some(0) => (0, first.start_tick),
            Some(index) => (
                self.sections[index - 1].start_tick,
                self.sections[index].start_tick,
            ),
            None => {
                let start = self.sections.last().map_or(0, |s| s.start_tick);
                let end = self.end_tick();
                let end = if end > start {
                    end
                } else {
                    start.saturating_add(TICKS_PER_BAR)
                };
                (start, end)
            }
        };
        Some([start, end.min(MAX_TICKS)])
    }

    pub fn frames_per_tick(&self, sample_rate: u32) -> f64 {
        sample_rate as f64 * 240.0 / self.bpm / TICKS_PER_BAR as f64
    }

    /// MixJam v7 stores an exact source span, represented by a derived tempo.
    /// Its importer reserves the `MixJam` group and retains its source-controls
    /// notice. Both survive older native saves; a group name alone is not proof.
    /// Placeholders and conflict choices also retain an explicit span policy.
    pub fn sample_uses_exact_tempo(&self, sample_id: u32) -> bool {
        self.missing_sample(sample_id).is_some_and(|sample| {
            sample
                .metadata
                .get("Playback timing policy")
                .is_some_and(|policy| policy == "mixjam-span")
        }) || self
            .sample_conflicts
            .iter()
            .any(|conflict| conflict.source_spans.contains_key(&sample_id))
            || (self
                .source_notices
                .iter()
                .any(|notice| notice.starts_with("MixJam v7 source controls: "))
                && self
                    .imported_samples
                    .iter()
                    .any(|sample| sample.id == sample_id && sample.group_name == "MixJam"))
    }

    /// The source clock used by rendering, placement, and musical trim controls.
    pub fn sample_playback_bpm(&self, sample_id: u32, source_bpm: f64) -> f64 {
        if self.sample_uses_exact_tempo(sample_id) {
            source_bpm
        } else {
            playback_bpm(source_bpm, self.bpm)
        }
    }
}

/// Select the nearest supported project tempo. Midpoint ties select the lower tempo.
pub fn nearest_supported_bpm(source_bpm: f64) -> Result<f64, String> {
    if !source_bpm.is_finite() || source_bpm <= 0.0 {
        return Err("The source tempo must be finite and positive.".into());
    }
    for pair in SUPPORTED_BPMS.windows(2) {
        if source_bpm <= (pair[0] + pair[1]) / 2.0 {
            return Ok(pair[0]);
        }
    }
    Ok(SUPPORTED_BPMS[SUPPORTED_BPMS.len() - 1])
}

/// The tempo at which a sound plays in a song at `song_bpm`. A sound's tempo
/// holds only up to octaves: a 90 BPM loop is a 180 BPM loop at half time.
/// Playback reads it in the octave nearest the song tempo, so no sound is sped
/// up or slowed down by more than half an octave.
pub fn playback_bpm(source_bpm: f64, song_bpm: f64) -> f64 {
    if !source_bpm.is_finite() || !song_bpm.is_finite() || source_bpm <= 0.0 || song_bpm <= 0.0 {
        return source_bpm;
    }
    source_bpm * (song_bpm / source_bpm).log2().round().exp2()
}
