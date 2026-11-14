use super::*;

#[test]
fn a_lane_compressor_works_before_the_fader_and_off_changes_nothing() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, tone(220.0))]);
    let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
    let frames = 11_025..44_100;
    let before = render(&project);
    let open = level_db(&before, frames.clone());
    // Settings that the compressor keeps while it is off change nothing.
    project.tracks[0].compressor = crate::compressor::Compressor {
        threshold: -30.0,
        ratio: 4.0,
        attack: 1.0,
        ..Default::default()
    };
    assert!(render(&project) == before);
    // The tone's mean power is 9 dB under full scale: 21 dB over the
    // threshold at 4:1 comes down by about 15.75 dB.
    project.tracks[0].compressor.on = true;
    let compressed = level_db(&render(&project), frames.clone());
    let change = compressed - open;
    assert!((change + 15.75).abs() < 1.0, "{change:.2} dB");
    // The fader follows the compressor, so it does not change how hard
    // the compressor works: 12 dB down on the fader is 12 dB down.
    project.tracks[0].gain = crate::engine::db_to_amplitude(-12.0);
    let lower = level_db(&render(&project), frames.clone()) - compressed;
    assert!((lower + 12.0).abs() < 0.05, "{lower:.3} dB");
    // The makeup gain raises the result.
    project.tracks[0].compressor.makeup = 6.0;
    let made_up = level_db(&render(&project), frames) - compressed;
    assert!((made_up + 6.0).abs() < 0.05, "{made_up:.3} dB");
}

#[test]
fn the_second_peak_cuts_its_line_in_the_mix() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, tone(191.0))]);
    let frames = 11_025..44_100;
    let level = |project: &Project| {
        level_db(
            &render_with_samples(project, &samples).unwrap().samples,
            frames.clone(),
        )
    };
    let open = level(&project);
    project.tracks[0].eq = crate::eq::ChannelEq {
        on: true,
        mid_db: 6.0,
        mid_hz: 2000.0,
        peak2_hz: 191.0,
        peak2_db: -12.0,
        peak2_q: 8.0,
        ..Default::default()
    };
    let cut = level(&project) - open;
    assert!((cut + 12.0).abs() < 0.5, "{cut:.2} dB");
}

#[test]
fn a_ducker_lowers_its_track_while_its_key_sounds_and_only_then() {
    // Track 1 holds a tone from the start; track 2's key enters halfway
    // through the bar, at frame 37,800, while the tone still sounds.
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 128, 128).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
    let alone = |project: &Project, track: usize| {
        let mix = mix_with_samples(project, &samples).unwrap();
        let mut out = vec![0.0; mix.frames() as usize * 2];
        mix.mix(0, &mut out, None, &mut RenderState::default(), Some(track));
        out
    };
    let open = alone(&project, 0);
    let key = alone(&project, 1);
    project.tracks[0].ducker = crate::ducker::Ducker {
        on: true,
        key: 2,
        threshold: -40.0,
        depth: 12.0,
        attack: 0.1,
        release: 20.0,
    };
    let ducked = alone(&project, 0);
    // Before the key: unchanged. With the key 34 dB over the threshold:
    // the depth, 12 dB down.
    assert_eq!(ducked[..37_800 * 2], open[..37_800 * 2]);
    let down = level_db(&ducked, 39_000..44_000) - level_db(&open, 39_000..44_000);
    assert!((down + 12.0).abs() < 0.2, "{down:.2} dB");
    // The key sounds as before: a key is measured, not changed.
    assert_eq!(alone(&project, 1), key);
    // A muted key does not duck.
    project.tracks[1].muted = true;
    assert_eq!(alone(&project, 0), open);
}

#[test]
fn the_master_gain_does_not_change_how_far_a_ducker_ducks() {
    // The key sits a few dB over the threshold, inside the depth, where
    // any change in its level would change the ducking.
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 128, 128).unwrap();
    let mut project = editor.project().clone();
    project.tracks[0].ducker = crate::ducker::Ducker {
        on: true,
        key: 2,
        threshold: -12.0,
        depth: 24.0,
        attack: 0.1,
        release: 20.0,
    };
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
    let ducked = |project: &Project| {
        let mix = mix_with_samples(project, &samples).unwrap();
        let mut out = vec![0.0; mix.frames() as usize * 2];
        mix.mix(0, &mut out, None, &mut RenderState::default(), Some(0));
        level_db(&out, 39_000..44_000)
    };
    project.master_gain = 1.0;
    let full = ducked(&project);
    project.master_gain = 0.5;
    let half = ducked(&project);
    // Half the gain is 6.02 dB down, and nothing more.
    assert!((half - full + 6.0206).abs() < 0.01, "{:.3} dB", half - full);
}

#[test]
fn a_tiny_master_gain_does_not_poison_the_ducker_after_the_key_stops() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.master_gain = 1e-39;
    project.tracks[0].ducker = crate::ducker::Ducker {
        on: true,
        key: 2,
        threshold: -12.0,
        depth: 24.0,
        attack: 0.1,
        release: 20.0,
    };
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let live = MasterControl::new(&project.master);
    live.set_levels(&project);
    let mut state = RenderState {
        live: Some(live.clone()),
        ..Default::default()
    };
    let mut block = vec![0.0; 2048];
    mix.mix(0, &mut block, None, &mut state, Some(0));
    project.master_gain = 1.0;
    project.tracks[1].gain = 0.0;
    live.set_levels(&project);
    let mut after = vec![0.0; 32_768 * 2];
    for (index, part) in after.chunks_mut(2048).enumerate() {
        mix.mix(1024 * (index + 1) as u64, part, None, &mut state, Some(0));
    }
    project.tracks[0].ducker.on = false;
    let mut expected = vec![0.0; after.len()];
    mix_with_samples(&project, &samples).unwrap().mix(
        1024,
        &mut expected,
        None,
        &mut RenderState::default(),
        Some(0),
    );
    let difference = level_db(&after, 28_672..32_768) - level_db(&expected, 28_672..32_768);
    assert!(
        difference.abs() < 0.1,
        "Ducker stayed reduced: {difference} dB"
    );
}

#[test]
fn playback_hands_a_live_master_gain_to_a_ducker_key_without_a_step() {
    // A master-gain edit during playback stores new levels without a
    // new mix: the key must take the live gain, glided as the levels
    // glide, and duck as an export at the new gain does.
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.tracks[0].ducker = crate::ducker::Ducker {
        on: true,
        key: 2,
        threshold: -12.0,
        depth: 24.0,
        attack: 0.1,
        release: 20.0,
    };
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
    project.master_gain = 1.0;
    let mix = mix_with_samples(&project, &samples).unwrap();
    let live = MasterControl::new(&project.master);
    live.set_levels(&project);
    let mut state = RenderState {
        live: Some(Arc::clone(&live)),
        ..RenderState::default()
    };
    let mut played = vec![0.0; 3 * 2048];
    mix.read_block(0, &mut played[..2048], None, &mut state);
    project.master_gain = 0.5;
    live.set_levels(&project);
    mix.read_block(1024, &mut played[2048..4096], None, &mut state);
    mix.read_block(2048, &mut played[4096..], None, &mut state);
    let mut expected = vec![0.0; played.len()];
    mix_with_samples(&project, &samples).unwrap().read_block(
        0,
        &mut expected,
        None,
        &mut RenderState::default(),
    );
    // Playback is the export at 0.5 times the gain it plays at over the
    // export's: 2, then a glide to 1 over the second block, then 1.
    for frame in 0..3072 {
        let along = (frame as f32 - 1023.0).clamp(0.0, 1024.0) / 1024.0;
        let scale = 2.0 - along;
        for channel in 0..2 {
            let at = 2 * frame + channel;
            assert!(
                (played[at] - scale * expected[at]).abs() < 1e-4,
                "frame {frame}: played {}, expected {}",
                played[at],
                scale * expected[at]
            );
        }
    }
}

/// The first frame of bar 3, 11 264 frames in: far from bar 1, and late
/// enough in the bar for the export's compressor to have settled.
fn inside_bar_3(project: &Project) -> usize {
    (512.0 * project.frames_per_tick(Mix::SAMPLE_RATE)).round() as usize + 11_264
}

#[test]
fn review_a_loop_at_zero_keeps_compression_and_off_on_discards_it() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.tracks[0].compressor.on = true;
    project.tracks[0].compressor.threshold = -30.0;
    let samples = HashMap::from([(1, tone(220.0))]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let live = MasterControl::new(&project.master);
    live.set_levels(&project);
    let mut state = RenderState {
        live: Some(live.clone()),
        loop_region: Some([0, 22_528]),
        ..Default::default()
    };
    let mut warm = vec![0.0; 22_528 * 2];
    mix.read_block(0, &mut warm, None, &mut state);
    let mut looped = vec![0.0; 1024 * 2];
    mix.read_block(0, &mut looped, None, &mut state);
    let difference = level_db(&looped, 0..256) - level_db(&warm, 0..256);
    assert!(difference < -6.0, "Loop attack restarted: {difference} dB");
    assert!(live.kept_compressor(0).is_some());
    project.tracks[0].compressor.on = false;
    live.set_levels(&project);
    project.tracks[0].compressor.on = true;
    live.set_levels(&project);
    assert!(
        live.kept_compressor(0).is_none(),
        "Off/on retained old reduction"
    );
}

#[test]
fn playback_compressors_carry_on_only_where_playback_continues() {
    // Bar 1 holds a tone that the compressor holds about 15.75 dB down,
    // with the default 10 ms attack. Bar 3 holds it 40 dB lower, under
    // the threshold and its knee.
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(1, 0, 512, 256).unwrap();
    let mut project = editor.project().clone();
    let quiet_clip = project.clips.iter_mut().find(|clip| clip.start_tick == 512);
    quiet_clip.unwrap().gain = 0.01;
    project.tracks[0].compressor = crate::compressor::Compressor {
        on: true,
        threshold: -30.0,
        ratio: 4.0,
        ..Default::default()
    };
    let samples = HashMap::from([(1, tone(220.0))]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let quiet = inside_bar_3(&project);
    let mut export = vec![0.0; (quiet + 2048) * 2];
    mix.read_block(0, &mut export, None, &mut RenderState::default());
    let live = MasterControl::new(&project.master);
    live.set_levels(&project);
    let playback = |loop_region: Option<[u64; 2]>| RenderState {
        live: Some(Arc::clone(&live)),
        loop_region,
        ..RenderState::default()
    };
    // The first 6 ms of a block against the export's same frames.
    let onset = |first: usize, block: &[f32]| {
        level_db(block, 0..256) - level_db(&export[first * 2..], 0..256)
    };
    let mut state = playback(Some([11_264, 22_528]));
    let mut block = vec![0.0; 2048];
    for first in (0..22_528).step_by(1024) {
        mix.read_block(first as u64, &mut block, None, &mut state);
    }
    // A mix prepared again, such as for a compressor dial's step, plays
    // through a new source with a new state from the playhead, which is
    // behind the frames that playback has rendered.
    mix.read_block(21_000, &mut block, None, &mut playback(None));
    let resumed = onset(21_000, &block);
    assert!(resumed.abs() < 0.5, "new mix: {resumed:.2} dB");
    // A loop pass, from the loop's end back to its start.
    mix.read_block(11_264, &mut block, None, &mut state);
    let looped = onset(11_264, &block);
    assert!(looped.abs() < 0.5, "loop pass: {looped:.2} dB");
    // A start far from where playback kept the history, here in the
    // quiet bar, begins from silence, as export has it there. The loud
    // bar's history would hold the track about 15 dB too low.
    mix.read_block(quiet as u64, &mut block, None, &mut playback(None));
    let far = onset(quiet, &block);
    assert!(far.abs() < 0.5, "far start: {far:.2} dB");
    // In the loud bar a far start begins without reduction, louder
    // until the attack catches up, as before playback kept histories.
    mix.read_block(5_000, &mut block, None, &mut playback(None));
    let fresh = onset(5_000, &block);
    assert!(fresh > 3.0, "fresh start: {fresh:.2} dB");
    // Playback from the song's start begins from silence, as export does.
    let mut from_start = vec![0.0; 2048];
    mix.read_block(0, &mut from_start, None, &mut playback(None));
    assert!(from_start[..] == export[..2048]);
}

#[test]
fn a_compressed_key_carries_its_tracks_history_and_starts_fresh_elsewhere() {
    // Track 3 keys track 1's ducker and is compressed: about 15.75 dB
    // down in bar 1, and untouched in bar 3, where it plays 30 dB lower.
    // The ducker works inside its depth in both bars, so any change in
    // the key's level changes how far track 1 ducks.
    let mut editor = crate::engine::Editor::new(Project::default());
    for start in [0, 512] {
        editor.place(1, 0, start, 256).unwrap();
        editor.place(2, 2, start, 256).unwrap();
    }
    let mut project = editor.project().clone();
    let quiet_key = project
        .clips
        .iter_mut()
        .find(|clip| clip.track == 2 && clip.start_tick == 512);
    quiet_key.unwrap().gain = 0.03;
    project.tracks[2].compressor = crate::compressor::Compressor {
        on: true,
        threshold: -30.0,
        ratio: 4.0,
        ..Default::default()
    };
    project.tracks[0].ducker = crate::ducker::Ducker {
        on: true,
        key: 3,
        threshold: -42.0,
        depth: 24.0,
        attack: 0.1,
        release: 20.0,
    };
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(1000.0))]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    // The mix prepared again has its chains in another order, so the
    // kept history must follow the track, not the chain's place.
    let mut reordered = project.clone();
    reordered
        .clips
        .sort_by_key(|clip| std::cmp::Reverse(clip.track));
    let again = mix_with_samples(&reordered, &samples).unwrap();
    let order = |mix: &Mix| {
        mix.chains
            .iter()
            .map(|chain| chain.track)
            .collect::<Vec<_>>()
    };
    assert_eq!((order(&mix), order(&again)), (vec![0, 2], vec![2, 0]));
    // Track 1 alone, as export plays it.
    let quiet = inside_bar_3(&project);
    let mut export = vec![0.0; (quiet + 2048) * 2];
    again.mix(0, &mut export, None, &mut RenderState::default(), Some(0));
    // 12 ms of a block after its first 128 frames, against the export's
    // same frames. The ducker's own history starts from silence, so its
    // attack takes those first frames whatever the key does.
    let onset = |first: usize, block: &[f32]| {
        level_db(block, 128..640) - level_db(&export[first * 2..], 128..640)
    };
    let live = MasterControl::new(&project.master);
    live.set_levels(&project);
    let playback = || RenderState {
        live: Some(Arc::clone(&live)),
        ..RenderState::default()
    };
    let mut state = playback();
    let mut block = vec![0.0; 2048];
    for first in (0..22_528).step_by(1024) {
        mix.read_block(first as u64, &mut block, None, &mut state);
    }
    // From the playhead, the key chain carries the key track's history,
    // so track 1 ducks as in export rather than deeper.
    again.mix(21_000, &mut block, None, &mut playback(), Some(0));
    let resumed = onset(21_000, &block);
    assert!(resumed.abs() < 0.5, "new mix: {resumed:.2} dB");
    // A start in the quiet bar begins fresh. A key carried from the loud
    // bar would sit about 15 dB low, and track 1 would duck that much
    // less: louder than export.
    again.mix(quiet as u64, &mut block, None, &mut playback(), Some(0));
    let far = onset(quiet, &block);
    assert!(far.abs() < 0.5, "far start: {far:.2} dB");
}

#[test]
fn playback_key_audio_is_allocated_before_the_first_read() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.master.bus_compressor.on = true;
    project.master.bus_compressor.key = 2;
    let samples = HashMap::from([(1, tone(220.0))]);
    let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let mut playback = PlaybackSource::new(mix, 0, None, Some(MasterControl::new(&project.master)));
    let capacity = playback.state.key_audio.capacity();
    let allocation = playback.state.key_audio.as_ptr();
    assert!(playback.fill());
    assert_eq!(playback.state.key_audio.capacity(), capacity);
    assert_eq!(playback.state.key_audio.as_ptr(), allocation);
}

#[test]
fn a_keyed_bus_compressor_lowers_the_mix_while_its_key_track_sounds() {
    // Track 2's key enters halfway through the bar, at frame 37,800.
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(2, 1, 128, 128).unwrap();
    let mut project = editor.project().clone();
    let samples = HashMap::from([(1, tone(220.0)), (2, tone(60.0))]);
    // At a 0 dBFS threshold the mix never reaches the knee.
    project.master.bus_compressor = crate::master::BusCompressor {
        on: true,
        threshold: 0.0,
        ratio: 10.0,
        attack: 0.1,
        release: 50.0,
        key: 0,
    };
    let render = |project: &Project| render_with_samples(project, &samples).unwrap().samples;
    let open = render(&project);
    project.master.bus_compressor.threshold = -30.0;
    project.master.bus_compressor.key = 2;
    let keyed = render(&project);
    // Before the key the mix is untouched; with it, the whole mix ducks.
    assert_eq!(keyed[..37_800 * 2], open[..37_800 * 2]);
    let down = level_db(&keyed, 39_000..44_000) - level_db(&open, 39_000..44_000);
    assert!(down < -12.0, "{down:.2} dB");
    // Playback hears the same key.
    let mix: Arc<dyn Frames> = Arc::new(mix_with_samples(&project, &samples).unwrap());
    let played: Vec<f32> =
        PlaybackSource::new(mix, 0, None, Some(MasterControl::new(&project.master))).collect();
    let played_down = level_db(&played, 39_000..44_000) - level_db(&open, 39_000..44_000);
    assert!(
        (played_down - down).abs() < 0.5,
        "{played_down:.2} against {down:.2} dB"
    );
    // A muted key is silent, so nothing ducks.
    project.tracks[1].muted = true;
    let muted = render(&project);
    project.master.bus_compressor.key = 0;
    project.master.bus_compressor.threshold = 0.0;
    assert_eq!(muted, render(&project));
}
