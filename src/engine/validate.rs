//! What a project must satisfy to be played, saved, or opened.
use super::{
    MAX_CLIPS, MAX_KEYS, MAX_PITCH_CENTS, MAX_TICKS, PROJECT_SCHEMA_VERSION, Project,
    SUPPORTED_BPMS, TRACK_COUNT, TRACK_WIDTH, sections_error, validate_import_metadata,
};
use std::collections::HashSet;

impl Project {
    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != PROJECT_SCHEMA_VERSION {
            return Err("This project version is not supported.".into());
        }
        if self.tracks.len() != TRACK_COUNT {
            return Err("The project must have 64 track slots.".into());
        }
        if !SUPPORTED_BPMS.contains(&self.bpm) {
            return Err("Tempo must be 120, 125, 130, 135, 140, 160, or 180 BPM.".into());
        }
        if self.pitch_ratio != 1.0 {
            return Err("Project pitch must be 1.0.".into());
        }
        if self.import_notices.iter().any(|notice| {
            !notice.source_bpm.is_finite()
                || notice.source_bpm <= 0.0
                || !SUPPORTED_BPMS.contains(&notice.target_bpm)
        }) {
            return Err("The project has invalid import tempo data.".into());
        }
        if self
            .source_settings
            .iter()
            .any(|(key, value)| key.is_empty() || key.contains('\0') || value.contains('\0'))
        {
            return Err("The project has invalid saved source settings.".into());
        }
        let mut library_products = HashSet::new();
        if self.libraries.iter().any(|binding| {
            binding.root.as_os_str().is_empty() || !library_products.insert(binding.product)
        }) {
            return Err("Each product needs one library binding with a nonempty root.".into());
        }
        if !valid_mixer_gain(self.master_gain) {
            return Err("Master amplitude must be from 0 to 4.".into());
        }
        self.master.validate()?;
        self.fx.validate()?;
        for (bus, lane) in self.return_automation.iter().enumerate() {
            crate::automation::validate_return_points(lane)
                .map_err(|error| format!("FX {}: {error}", bus + 1))?;
        }
        if self
            .tracks
            .iter()
            .any(|track| !valid_mixer_gain(track.gain))
        {
            return Err("Track amplitude must be from 0 to 4.".into());
        }
        if self
            .tracks
            .iter()
            .any(|track| !track.pan.is_finite() || !(-1.0..=1.0).contains(&track.pan))
        {
            return Err("Track pan must be from -1 to 1.".into());
        }
        if self.tracks.iter().any(|track| {
            track
                .sends
                .iter()
                .any(|send| !send.is_finite() || !(0.0..=1.0).contains(send))
        }) {
            return Err("Track sends must be from 0 to 1.".into());
        }
        if self.tracks.iter().any(|track| {
            !track.width.is_finite() || !(TRACK_WIDTH.low..=TRACK_WIDTH.high).contains(&track.width)
        }) {
            return Err("Track width must be from 0 to 200%.".into());
        }
        for (index, track) in self.tracks.iter().enumerate() {
            track
                .automation
                .validate()
                .and_then(|()| track.eq.validate())
                .and_then(|()| track.drive.validate())
                .and_then(|()| track.compressor.validate())
                .and_then(|()| track.ducker.validate(index))
                .map_err(|error| format!("Track {}: {error}", index + 1))?;
        }
        let mut keys: Vec<usize> = self
            .tracks
            .iter()
            .filter(|track| track.ducker.on)
            .map(|track| track.ducker.key_track())
            .chain(self.side_keys())
            .collect();
        keys.sort_unstable();
        keys.dedup();
        if keys.len() > MAX_KEYS {
            return Err(format!(
                "The duckers, master pedals, and FX buses can use at most {MAX_KEYS} tracks \
                 as keys."
            ));
        }
        if self.clips.len() > MAX_CLIPS {
            return Err("The project has too many clips.".into());
        }
        let mut ids = HashSet::new();
        let mut imported_ids = HashSet::new();
        // The open project holds the whole Sounds panel as imports, so its
        // count follows the library. The song and project readers' byte
        // limits bound what a file can declare.
        for sample in &self.imported_samples {
            validate_import_metadata(sample)?;
            if !imported_ids.insert(sample.id) {
                return Err("Imported sample IDs must be unique.".into());
            }
        }
        if self.missing_samples.len() > 100_000 {
            return Err("The project has too many missing samples.".into());
        }
        for sample in &self.missing_samples {
            sample.validate()?;
            if !imported_ids.insert(sample.id) {
                return Err("Resolved and missing sample IDs must be unique.".into());
            }
        }
        if self.sample_conflicts.len() > 100_000 {
            return Err("The project has too many sample conflicts.".into());
        }
        let mut conflict_ids = HashSet::new();
        for conflict in &self.sample_conflicts {
            let mut paths = HashSet::new();
            let source_ids: HashSet<_> = conflict.sample_ids.iter().copied().collect();
            if conflict.reference.trim().is_empty()
                || conflict.reference.len() > 65_536
                || conflict.reference.contains('\0')
                || conflict.sample_ids.is_empty()
                || conflict.sample_ids.len() > 100_000
                || !(2..=100_000).contains(&conflict.candidates.len())
                || conflict.selected >= conflict.candidates.len()
                || conflict.candidates.iter().any(|path| {
                    path.as_os_str().is_empty()
                        || path.to_string_lossy().contains('\0')
                        || !paths.insert(path)
                })
                || conflict.sample_ids.iter().any(|id| {
                    !conflict_ids.insert(*id)
                        || (!imported_ids.contains(id)
                            && crate::products::Product::sample_key(*id).is_none())
                })
                || conflict
                    .source_spans
                    .iter()
                    .any(|(id, ticks)| !source_ids.contains(id) || !(1..=MAX_TICKS).contains(ticks))
            {
                return Err(
                    "The sample conflict has invalid references, choices, or timing.".into(),
                );
            }
        }
        for c in &self.clips {
            if c.sample_id >= crate::products::IMPORTED_ID_START
                && !imported_ids.contains(&c.sample_id)
            {
                return Err(format!("Clip {} has no imported sample reference.", c.id));
            }
            if !ids.insert(c.id) {
                return Err("Clip IDs must be unique.".into());
            }
            if c.track >= TRACK_COUNT
                || c.length_ticks == 0
                || c.start_tick
                    .checked_add(c.length_ticks)
                    .is_none_or(|end| end > MAX_TICKS)
                || !valid_gain(c.gain)
            {
                return Err(format!(
                    "Clip {} has invalid track, time, or volume data.",
                    c.id
                ));
            }
            if c.fades.fit(c.length_ticks) != c.fades {
                return Err(format!("Clip {} has fades longer than the clip.", c.id));
            }
            if !(-MAX_PITCH_CENTS..=MAX_PITCH_CENTS).contains(&c.pitch_cents) {
                return Err(format!(
                    "Clip {} pitch must be from -24 to 24 semitones.",
                    c.id
                ));
            }
        }
        if let Some([a, b]) = self.loop_region
            && (a >= b || b > MAX_TICKS)
        {
            return Err("The loop must be within the timeline and have a positive length.".into());
        }
        if let Some(error) = sections_error(&self.sections) {
            return Err(error);
        }
        Ok(())
    }
}

pub(super) fn valid_gain(g: f32) -> bool {
    g.is_finite() && (0.0..=2.0).contains(&g)
}

fn valid_mixer_gain(g: f32) -> bool {
    // Earlier tracks could reach exactly 4.0 (+12.041 dB). Retain that value
    // when reopening a song; ordinary mixer controls stop at +12 dB.
    g.is_finite() && (0.0..=4.0).contains(&g)
}
