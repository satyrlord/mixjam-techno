//! Source tempos authenticated by original decoded audio, independent of paths.
use crate::{catalog, library::WavProbe, products::Product};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct SourceTempos(HashMap<[u8; 16], Option<f64>>);

impl SourceTempos {
    pub(crate) fn bundled() -> Result<Self, String> {
        let mut result = Self::default();
        for product in Product::ALL {
            for entry in catalog::bundled_wave_metadata(product)? {
                if let Some(digest) = entry.pcm_identity().filter(|_| entry.frame_count > 2) {
                    result.insert(digest, entry.source_bpm);
                }
            }
        }
        let extra: Vec<([u8; 16], f64)> = serde_json::from_str(include_str!(concat!(
            env!("OUT_DIR"),
            "/source-tempos.json"
        )))
        .map_err(|error| format!("Cannot read bundled source tempos: {error}"))?;
        for (digest, bpm) in extra {
            if !bpm.is_finite() || bpm <= 0.0 {
                return Err("A bundled source tempo is invalid.".into());
            }
            result.insert(digest, bpm);
        }
        Ok(result)
    }

    fn insert(&mut self, digest: [u8; 16], bpm: f64) {
        self.0
            .entry(digest)
            .and_modify(|known| {
                if *known != Some(bpm) {
                    *known = None;
                }
            })
            .or_insert(Some(bpm));
    }

    /// Require three authenticated files, at least 75 percent coverage, and
    /// agreement among all authenticated tempos. Mixed or unknown folders keep
    /// their duration inference. An identity used at different original tempos
    /// cannot establish a tempo.
    pub(crate) fn folder<'a>(&self, probes: impl Iterator<Item = &'a WavProbe>) -> Option<f64> {
        let mut total = 0;
        let mut matched = 0;
        let mut tempo = None;
        for probe in probes {
            total += 1;
            if probe.channels != 1 || probe.sample_rate != 44_100 || probe.frame_count <= 2 {
                continue;
            }
            let Some(bpm) = probe
                .digest
                .and_then(|digest| self.0.get(&digest).copied().flatten())
            else {
                continue;
            };
            if tempo.is_some_and(|previous| previous != bpm) {
                return None;
            }
            tempo = Some(bpm);
            matched += 1;
        }
        (matched >= 3 && matched * 4 >= total * 3)
            .then_some(tempo)
            .flatten()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn probe(digest: Option<[u8; 16]>) -> WavProbe {
        WavProbe {
            frame_count: 117_600,
            sample_rate: 44_100,
            channels: 1,
            length: 235_244,
            digest,
        }
    }

    #[test]
    fn original_audio_resolves_octaves_without_folder_names_or_durations() {
        let mut evidence = SourceTempos::default();
        evidence.insert([1; 16], 90.0);
        evidence.insert([2; 16], 180.0);
        for (digest, expected) in [([1; 16], 90.0), ([2; 16], 180.0)] {
            let probes = [
                probe(Some(digest)),
                probe(Some(digest)),
                probe(Some(digest)),
                probe(None),
            ];
            assert_eq!(evidence.folder(probes.iter()), Some(expected));
            assert_eq!(evidence.folder(probes[..2].iter()), None);
            assert_eq!(
                evidence.folder(probes.iter().chain([probe(None)].iter())),
                None
            );
        }
        assert_eq!(evidence.folder([].iter()), None);
        assert_eq!(
            evidence.folder([probe(Some([3; 16])), probe(None), probe(None)].iter()),
            None
        );
    }

    #[test]
    fn mixed_and_conflicting_original_tempos_do_not_override_inference() {
        let mut evidence = SourceTempos::default();
        evidence.insert([1; 16], 90.0);
        evidence.insert([2; 16], 180.0);
        let mut probes = vec![probe(Some([1; 16])); 20];
        probes.push(probe(Some([2; 16])));
        assert_eq!(evidence.folder(probes.iter()), None);
        evidence.insert([1; 16], 180.0);
        evidence.insert([1; 16], 90.0);
        assert_eq!(evidence.folder(probes[..20].iter()), None);
    }

    #[test]
    fn incompatible_cached_probe_digests_do_not_establish_tempo() {
        let mut evidence = SourceTempos::default();
        evidence.insert([1; 16], 90.0);
        for (rate, channels, frames) in [(48_000, 1, 117_600), (44_100, 2, 117_600), (44_100, 1, 2)]
        {
            let mut invalid = probe(Some([1; 16]));
            invalid.sample_rate = rate;
            invalid.channels = channels;
            invalid.frame_count = frames;
            assert_eq!(evidence.folder([invalid; 1].iter().cycle().take(3)), None);
        }
    }
}
