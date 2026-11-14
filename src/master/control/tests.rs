use super::*;

#[test]
fn compressor_switches_invalidate_only_their_track_and_old_sources_cannot_restore_it() {
    let mut project = crate::engine::Project::default();
    project.tracks[0].compressor.on = true;
    project.tracks[1].compressor.on = true;
    let control = MasterControl::new(&project.master);
    control.set_levels(&project);
    let epoch = control.compressor_epoch();
    for track in 0..2 {
        control.keep_compressor(track, Default::default(), 10_000, epoch);
    }
    project.tracks[0].compressor.on = false;
    control.set_levels(&project);
    assert!(control.kept_compressor(0).is_none());
    assert!(control.kept_compressor(1).is_some());
    control.keep_compressor(0, Default::default(), 11_000, epoch);
    assert!(control.kept_compressor(0).is_none());
    control.reset_compressors();
    control.keep_compressor(1, Default::default(), 11_000, epoch);
    assert!(control.kept_compressor(1).is_none());
}

#[test]
fn live_levels_reject_an_incomplete_publication() {
    let project = crate::engine::Project::default();
    let control = MasterControl::new(&project.master);
    control.set_levels(&project);
    control.level_sequence.fetch_add(1, Ordering::SeqCst);
    control
        .master_gain
        .store(0.5_f32.to_bits(), Ordering::SeqCst);
    // The former independent reads accepted this torn publication.
    let packed = control.gain_and_pan[0].load(Ordering::SeqCst);
    let old_gain = f32::from_bits(packed as u32);
    let new_gain = f32::from_bits(control.master_gain.load(Ordering::SeqCst));
    assert_ne!(old_gain.abs(), new_gain);
    assert!(
        control
            .levels(&[false; crate::engine::TRACK_COUNT])
            .is_none()
    );
}

#[test]
fn live_stereo_levels_are_published_together() {
    let mut project = crate::engine::Project {
        master_gain: 1.0,
        ..Default::default()
    };
    project.tracks[0].pan = 1.0;
    let control = MasterControl::new(&project.master);
    assert_eq!(control.levels(&[false; crate::engine::TRACK_COUNT]), None);
    control.set_levels(&project);
    let done = std::sync::atomic::AtomicBool::new(false);
    std::thread::scope(|scope| {
        scope.spawn(|| {
            for _ in 0..100_000 {
                project.tracks[0].polarity_inverted = !project.tracks[0].polarity_inverted;
                project.master_gain = if project.tracks[0].polarity_inverted {
                    0.25
                } else {
                    1.0
                };
                project.tracks[0].pan = if project.tracks[0].polarity_inverted {
                    -1.0
                } else {
                    1.0
                };
                control.set_levels(&project);
            }
            done.store(true, std::sync::atomic::Ordering::Release);
        });
        while !done.load(std::sync::atomic::Ordering::Acquire) {
            for automated in [false, true] {
                let Some((levels, master_gain)) =
                    control.levels(&[automated; crate::engine::TRACK_COUNT])
                else {
                    continue;
                };
                let expected = if master_gain == 0.25 {
                    [-0.25, if automated { -0.25 } else { 0.0 }]
                } else {
                    [if automated { 1.0 } else { 0.0 }, 1.0]
                };
                assert_eq!(
                    levels[0], expected,
                    "Gain, pan, and master gain must share one publication."
                );
            }
        }
    });
}
