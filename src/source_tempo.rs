//! Source tempos authenticated by original decoded audio, independent of paths.
use crate::{catalog, library::WavProbe, products::Product};
use std::collections::HashMap;

#[derive(Default)]
pub(crate) struct SourceTempos(HashMap<[u8; 16], Option<f64>>);

impl SourceTempos {
    /// The tempos of the runtime catalogs and their `source-tempos.json`,
    /// gathered once for each reading of the catalogs.
    pub(crate) fn for_catalogs(catalogs: &'static catalog::Catalogs) -> &'static Self {
        type Shared = Option<(usize, &'static SourceTempos)>;
        static SHARED: std::sync::Mutex<Shared> = std::sync::Mutex::new(None);
        let reading = std::ptr::from_ref(catalogs) as usize;
        let mut shared = SHARED
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match *shared {
            Some((known, tempos)) if known == reading => tempos,
            _ => {
                let tempos: &'static Self = Box::leak(Box::new(Self::from_catalogs(catalogs)));
                *shared = Some((reading, tempos));
                tempos
            }
        }
    }

    pub(crate) fn from_catalogs(catalogs: &catalog::Catalogs) -> Self {
        let mut result = Self::default();
        for product in Product::ALL {
            for entry in catalogs.identities(product) {
                if let Some(digest) = entry.pcm_identity().filter(|_| entry.frame_count > 2) {
                    result.insert(digest, entry.source_bpm);
                }
            }
        }
        for (digest, bpm) in catalogs.source_tempos() {
            result.insert(*digest, *bpm);
        }
        result
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
    fn runtime_catalogs_and_their_source_tempos_both_give_evidence() {
        let folder =
            std::env::temp_dir().join(format!("mixjam-source-tempos-{}", std::process::id()));
        std::fs::create_dir_all(&folder).unwrap();
        let audio = [0, 3, 9, 4, 1];
        let sources = vec![catalog::SourceFile {
            bytes: 1,
            sha256: "00".repeat(32),
        }];
        let file = catalog::CatalogFile {
            schema: catalog::SCHEMA,
            product: Product::T1,
            release: "test".into(),
            fingerprint: catalog::fingerprint(&sources),
            sources,
            samples: vec![catalog::WaveMetadata {
                native_id: 1,
                code: "CODE".into(),
                label_key: crate::catalog::label_key("Sound"),
                group_name: "Loop".into(),
                bars: 1,
                source_bpm: 180.0,
                frame_count: 5,
                pcm_digest: Some(catalog::pcm_identity_hex(&audio)),
            }],
        };
        std::fs::write(folder.join("t1.json"), serde_json::to_vec(&file).unwrap()).unwrap();
        std::fs::write(
            folder.join(catalog::SOURCE_TEMPOS),
            serde_json::to_vec(&[([2_u8; 16], 90.0)]).unwrap(),
        )
        .unwrap();
        let catalogs = catalog::Catalogs::open(Some(folder.clone()));
        assert!(catalogs.problems().is_empty(), "{:?}", catalogs.problems());
        let evidence = SourceTempos::from_catalogs(&catalogs);
        let digest = catalog::pcm_identity(&audio);
        let probes = [
            probe(Some(digest)),
            probe(Some(digest)),
            probe(Some(digest)),
        ];
        assert_eq!(evidence.folder(probes.iter()), Some(180.0));
        let probes = [
            probe(Some([2; 16])),
            probe(Some([2; 16])),
            probe(Some([2; 16])),
        ];
        assert_eq!(evidence.folder(probes.iter()), Some(90.0));
        std::fs::remove_dir_all(folder).unwrap();
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
