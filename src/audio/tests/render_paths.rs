//! The samples of every render path, pinned by digest: export, export
//! without the buses, the stems, and playback in its block sizes, from a
//! later start, through a loop, and of decoded audio. A refactor of the
//! block schedule must keep each digest. A change that means to change the
//! sound updates them.
use super::*;

/// FNV-1a over the bits of each sample, seeded with the count.
fn digest(samples: &[f32]) -> u64 {
    samples.iter().fold(
        0xcbf2_9ce4_8422_2325 ^ samples.len() as u64,
        |hash, sample| (hash ^ u64::from(sample.to_bits())).wrapping_mul(0x0100_0000_01b3),
    )
}

/// The busy song with a drive on the key track of track 1's ducker. With
/// `later_ducker`, track 3 ducks instead of track 1, so the first stem does
/// not duck but still measures the master pedals' key.
fn fixture(later_ducker: bool) -> (Project, HashMap<u32, Pcm>) {
    let (mut project, samples) = super::playback::busy_song();
    project.tracks[1].drive = crate::drive::Drive {
        on: true,
        amount: 18.0,
        mix: 80.0,
        ..Default::default()
    };
    if later_ducker {
        project.tracks[2].ducker = project.tracks[0].ducker;
        project.tracks[2].ducker.key = 1;
        project.tracks[0].ducker.on = false;
    }
    (project, samples)
}

/// Live playback from `start` in blocks of `block` frames, until it ends
/// or `limit` samples have played.
fn play(
    mix: &Arc<Mix>,
    project: &Project,
    start: u64,
    loop_region: Option<[u64; 2]>,
    block: usize,
    limit: usize,
) -> Vec<f32> {
    let control = MasterControl::new(&project.master);
    control.set_returns(&project.fx);
    control.set_levels(project);
    let audio: Arc<dyn Frames> = mix.clone();
    PlaybackSource::with_block(audio, start, loop_region, Some(control), block)
        .take(limit)
        .collect()
}

/// Each part of the stems, whole.
fn stems(mix: &Mix, fx: &FxBuses) -> Vec<Vec<f32>> {
    let tracks = mix.sounding_tracks();
    let buses = mix.returning_buses(fx);
    let mut parts = vec![Vec::new(); tracks.len() + buses.len()];
    mix.render_stems(fx, &tracks, &buses, |index, block| {
        parts[index].extend_from_slice(block);
        Ok(())
    })
    .unwrap();
    parts
}

fn stems_sum_exact(mix: &Mix, project: &Project) {
    let parts = stems(mix, &project.fx);
    let raw_export = mix
        .to_pcm(&MasterChain::default(), &project.fx)
        .unwrap()
        .samples;
    let len = parts[0].len();
    assert_eq!(len, raw_export.len(), "stems length vs raw export");
    let mut sum = vec![0.0_f32; len];
    for part in &parts {
        assert_eq!(part.len(), len);
        for (dst, src) in sum.iter_mut().zip(part) {
            *dst += *src;
        }
    }
    let max_diff = sum
        .iter()
        .zip(&raw_export)
        .map(|(a, b)| (a - b).abs())
        .fold(0.0_f32, f32::max);
    assert!(
        max_diff <= 1e-5,
        "stems sum diverged from raw export by {max_diff}"
    );
}

fn check_live_offline_parity(played: &[f32], exported: &[f32]) {
    let n = played.len().min(exported.len());
    let rms = |s: &[f32]| -> f64 {
        (s.iter().map(|v| f64::from(*v).powi(2)).sum::<f64>() / n as f64)
            .sqrt()
            .max(1e-12)
    };
    let delta_db = 20.0 * (rms(&played[..n]) / rms(&exported[..n])).log10();
    assert!(
        delta_db.abs() <= 0.5,
        "Live vs Offline RMS delta {delta_db:.2} dB > 0.5 dB"
    );
    // Transient/onset alignment within <= 2 samples via short-lag cross-correlation.
    let window = (n / 2).min(4096);
    let best_lag = (-4_isize..=4)
        .max_by(|&a, &b| {
            let corr = |lag: isize| -> f64 {
                (8..window - 8)
                    .map(|i| {
                        let j = (i as isize + lag) as usize;
                        f64::from(played[i * 2]) * f64::from(exported[j * 2])
                    })
                    .sum()
            };
            corr(a).total_cmp(&corr(b))
        })
        .unwrap();
    assert!(best_lag.abs() <= 2, "transient skew {best_lag} > 2 samples");
}

fn digests() -> Vec<(String, u64)> {
    let mut digests = Vec::new();
    let mut add = |name: String, samples: &[f32]| digests.push((name, digest(samples)));
    for later_ducker in [false, true] {
        let (project, samples) = fixture(later_ducker);
        let mix = Arc::new(mix_with_samples(&project, &samples).unwrap());
        let live = Arc::new(
            mix_with_samples_quality(
                &project,
                &samples,
                crate::playback_quality::PlaybackQuality::FULL,
            )
            .unwrap(),
        );
        let name = |path: &str| format!("{path}, later ducker {later_ducker}");
        // The pedals delay the sound, and the buses ring on after the song.
        assert!(project.master.latency() > 0);
        let exported = mix.to_pcm(&project.master, &project.fx).unwrap().samples;
        assert!(exported.len() / 2 > mix.frames() as usize + 8 * Mix::TAIL_BLOCK as usize);
        add(name("export"), &exported);
        stems_sum_exact(&mix, &project);
        let silent = FxBuses::default();
        assert!(!silent.is_active());
        let dry = mix.to_pcm(&project.master, &silent).unwrap().samples;
        assert_eq!(dry.len(), mix.frames() as usize * 2);
        add(name("export without buses"), &dry);
        for (index, part) in stems(&mix, &project.fx).iter().enumerate() {
            add(name(&format!("stem {index}")), part);
        }
        let mut live_ref: Option<Vec<f32>> = None;
        for block in [PlaybackSource::BLOCK_FRAMES, 100] {
            let played = play(&live, &project, 0, None, block, usize::MAX);
            if let Some(first) = &live_ref {
                assert_eq!(&played, first, "{block}-frame live playback drifted");
            } else {
                check_live_offline_parity(&played, &exported);
                live_ref = Some(played.clone());
            }
            add(name(&format!("playback in {block}-frame blocks")), &played);
        }
        add(
            name("playback from a later start"),
            &play(&live, &project, 20_000, None, 256, usize::MAX),
        );
        add(
            name("playback through a loop"),
            &play(
                &live,
                &project,
                30_000,
                Some([10_000, 40_000]),
                256,
                200_000,
            ),
        );
        let mut read = vec![0.0; (mix.frames() as usize + 100) * 2];
        mix.read(0, &mut read);
        add(name("read"), &read);
    }
    // Decoded audio: through the board without buses, and alone.
    let (project, _) = fixture(false);
    let decoded: Arc<dyn Frames> = Arc::new(stereo(
        |i| (i as f32 * 0.01).sin() * 0.9,
        |i| (i as f32 * 0.017).cos() * 0.9,
    ));
    let boarded: Vec<f32> = PlaybackSource::with_block(
        Arc::clone(&decoded),
        0,
        None,
        Some(MasterControl::new(&project.master)),
        256,
    )
    .collect();
    add("decoded audio through the board".into(), &boarded);
    let alone: Vec<f32> = PlaybackSource::new(decoded, 0, None, None).collect();
    add("decoded audio alone".into(), &alone);
    digests
}

#[test]
fn every_render_path_keeps_its_samples() {
    let actual = digests();
    let expected: &[(&str, u64)] = &[
        ("export, later ducker false", 0xd6b1c55b45b465be),
        (
            "export without buses, later ducker false",
            0x974011af7875a611,
        ),
        ("stem 0, later ducker false", 0x02c153ddc99aa075),
        ("stem 1, later ducker false", 0x645043dcc90d21e9),
        ("stem 2, later ducker false", 0xbaf9536bb7dfac91),
        ("stem 3, later ducker false", 0x0359f54aba4bb5db),
        ("stem 4, later ducker false", 0xd1300d8928575d15),
        (
            "playback in 256-frame blocks, later ducker false",
            0x79dbe53e3eaa94d7,
        ),
        (
            "playback in 100-frame blocks, later ducker false",
            0x79dbe53e3eaa94d7,
        ),
        (
            "playback from a later start, later ducker false",
            0x565373350e0c91a8,
        ),
        (
            "playback through a loop, later ducker false",
            0x915872f8e62839ca,
        ),
        ("read, later ducker false", 0x25eee86543767d85),
        ("export, later ducker true", 0xff933c194f2c9eb5),
        (
            "export without buses, later ducker true",
            0xbc59443861417e61,
        ),
        ("stem 0, later ducker true", 0x73314d04b2dadcb3),
        ("stem 1, later ducker true", 0x432c2f71c4ae39e9),
        ("stem 2, later ducker true", 0x5d2a8e0722588667),
        ("stem 3, later ducker true", 0xe14c9ee76064d4e3),
        ("stem 4, later ducker true", 0x7128ce242f61a515),
        (
            "playback in 256-frame blocks, later ducker true",
            0xbfab1791fa414393,
        ),
        (
            "playback in 100-frame blocks, later ducker true",
            0xbfab1791fa414393,
        ),
        (
            "playback from a later start, later ducker true",
            0xada7fb4b80a58135,
        ),
        (
            "playback through a loop, later ducker true",
            0xd84b14e0788753b3,
        ),
        ("read, later ducker true", 0x3961a53880e25ff7),
        ("decoded audio through the board", 0xf70603c6afdbde4c),
        ("decoded audio alone", 0x91a1607d612e209f),
    ];
    let listed: String = actual
        .iter()
        .map(|(name, digest)| format!("(\"{name}\", {digest:#018x}),\n"))
        .collect();
    assert_eq!(
        actual.len(),
        expected.len(),
        "the digests are now:\n{listed}"
    );
    for ((name, digest), (expected_name, expected_digest)) in actual.iter().zip(expected) {
        assert_eq!(name, expected_name);
        assert_eq!(
            digest, expected_digest,
            "{name} changed; the digests are now:\n{listed}"
        );
    }
}
