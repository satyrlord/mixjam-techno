use super::board::LIVE_LATENCY;
use super::dsp::db_to_gain;
use super::*;

#[test]
fn playback_meter_ignores_the_delay_removed_from_the_audible_output() {
    let control = MasterControl::new(&MasterChain::default());
    let mut processor = MasterProcessor::live(control.clone());
    let frames = 17_640; // One complete 400 ms loudness block.
    let latency = processor.latency();
    let mut block = vec![0.0; (frames + latency) * 2];
    for frame in 0..frames {
        let value = (frame as f32 * std::f32::consts::TAU * 1000.0 / 44_100.0).sin() * 0.1;
        block[frame * 2..frame * 2 + 2].fill(value);
    }
    for chunk in block.chunks_mut(514) {
        processor.process(chunk);
    }
    // The end of playback publishes the frames since the last reading.
    processor.finish();
    let mut reference = meters::Loudness::new();
    for &[left, right] in block[latency * 2..].as_chunks::<2>().0 {
        reference.push(left, right);
    }
    assert!(reference.integrated.is_finite());
    assert!(
        (f64::from(control.meters().integrated) - reference.integrated).abs() < 1e-5,
        "playback {}, audible {}",
        control.meters().integrated,
        reference.integrated
    );
}

#[test]
fn meter_readings_do_not_depend_on_playbacks_block_size() {
    let read = |block_frames: usize| {
        let control = MasterControl::new(&MasterChain::default());
        let mut processor = MasterProcessor::live(control.clone());
        // Half a second over the meters' limit, then half a second under it.
        let mut audio: Vec<f32> = (0..44_100)
            .flat_map(|frame| {
                let level = if frame < 22_050 { 0.99 } else { 0.1 };
                let value = level * (frame as f32 * 0.07).sin();
                [value, value]
            })
            .collect();
        for block in audio.chunks_mut(block_frames * 2) {
            processor.process(block);
        }
        processor.finish();
        control.meters()
    };
    let whole = read(1024);
    assert!(whole.overs > 10, "{whole:?}");
    for block_frames in [256, 128, 100, 1536, 4096] {
        assert_eq!(read(block_frames), whole, "{block_frames}-frame blocks");
    }
}

fn interleaved(frames: &[(f32, f32)]) -> Vec<f32> {
    frames.iter().flat_map(|(l, r)| [*l, *r]).collect()
}

fn mastered() -> MasterChain {
    let mut chain = MasterChain::default();
    for kind in PedalKind::ALL.into_iter().filter(|kind| !kind.is_booster()) {
        chain.set_on(kind, true);
    }
    chain
}

#[test]
fn an_inactive_chain_leaves_the_mix_untouched() {
    let mut block = vec![0.123_456_7, -0.987_654_3, 1.0, -1.0];
    let original = block.clone();
    let mut chain = MasterChain::default();
    chain.booster_eq.bands = [100; 10];
    chain.limiter.gain = 12.0;
    MasterProcessor::new(&chain).process(&mut block);
    assert_eq!(block, original);
    assert!(!chain.is_active());

    // Any Booster pedal on gives 16-bit samples, as the original's effects
    // do; the mastering pedals keep full precision.
    let mut wide = chain;
    wide.booster_wide.on = true;
    wide.booster_wide.spread = 0;
    MasterProcessor::new(&wide).process(&mut block);
    assert!(block.iter().all(|v| (v * 32768.0).fract() == 0.0));
    let mut lift = chain;
    lift.lift_eq.on = true;
    let mut quiet = vec![0.123_456_7, -0.087_654_3];
    MasterProcessor::new(&lift).process(&mut quiet);
    assert!(quiet.iter().any(|v| (v * 32768.0).fract() != 0.0));

    // The trim alone changes the level.
    let mut trimmed = chain;
    trimmed.trim = -6.0;
    assert!(trimmed.is_active());
    let mut half = vec![0.5, -0.5];
    MasterProcessor::new(&trimmed).process(&mut half);
    assert!((half[0] - 0.5 * db_to_gain(-6.0) as f32).abs() < 1e-6);
}

#[test]
fn delaying_pedals_add_up_and_others_add_nothing() {
    let mut chain = MasterChain::default();
    assert_eq!(chain.latency(), 0);
    chain.booster_wide.on = true;
    chain.lift_eq.on = true;
    assert_eq!(chain.latency(), 0);
    chain.soft_clip.on = true;
    chain.limiter.on = true;
    assert_eq!(
        MasterProcessor::new(&chain).latency(),
        pedals::SoftClip::LATENCY + pedals::Limiter::LATENCY
    );
    assert_eq!(
        mastered().latency(),
        4 * dsp::Oversampler::LATENCY + pedals::Multiband::LATENCY + pedals::Limiter::LATENCY
    );
}

#[test]
fn master_playback_quality_preserves_delay_safety_and_full_quality_samples() {
    use crate::playback_quality::Oversampling;
    let input: Vec<f32> = (0..5000)
        .flat_map(|frame| {
            let x = (frame as f32 * 0.71).sin() * 0.8;
            [x, -0.4 * x]
        })
        .collect();
    let saturation = [
        PedalKind::SoftClip,
        PedalKind::Tube,
        PedalKind::Maximizer,
        PedalKind::Tape,
    ];
    for mask in 0..16 {
        let mut chain = MasterChain::default();
        for (bit, pedal) in saturation.into_iter().enumerate() {
            chain.set_on(pedal, mask & (1 << bit) != 0);
        }
        chain.soft_clip.amount = 12.0;
        chain.tube.drive = 10.0;
        chain.maximizer.boost = 50.0;
        chain.tape.drive = 10.0;
        // Detector oversampling and lookahead remain enabled in both modes.
        chain.limiter.on = true;
        chain.multiband.on = true;
        let mut full = MasterProcessor::new(&chain);
        let mut explicit = MasterProcessor::with_quality(&chain, Oversampling::FourX);
        let mut reduced = MasterProcessor::with_quality(&chain, Oversampling::Off);
        assert_eq!(full.latency(), reduced.latency());
        let mut expected = input.clone();
        let mut actual = input.clone();
        let mut off = input.clone();
        full.process(&mut expected);
        for chunk in actual.chunks_mut(74) {
            explicit.process(chunk);
        }
        for chunk in off.chunks_mut(74) {
            reduced.process(chunk);
        }
        assert_eq!(actual, expected, "full quality mask {mask}");
        assert!(
            off.iter()
                .all(|value| value.is_finite() && value.abs() <= 1.0)
        );
        if mask == 0 {
            assert_eq!(off, expected);
        } else {
            assert_ne!(off, expected, "saturation mask {mask}");
        }
        for quality in [Oversampling::FourX, Oversampling::Off] {
            let control = MasterControl::new(&chain);
            let mut live = MasterProcessor::live_with_quality(control.clone(), quality);
            assert_eq!(live.latency(), LIVE_LATENCY);
            let mut block = input.clone();
            live.process(&mut block);
            chain.soft_clip.on = !chain.soft_clip.on;
            control.set(&chain);
            live.process(&mut block);
            assert_eq!(live.latency(), LIVE_LATENCY);
            assert!(block.iter().all(|x| x.is_finite()));
            chain.soft_clip.on = !chain.soft_clip.on;
        }
    }
}

#[test]
fn returning_to_a_track_key_does_not_replay_its_old_audio() {
    let mut chain = MasterChain::default();
    chain.soft_clip.on = true;
    chain.soft_clip.amount = 0.0;
    chain.bus_compressor = BusCompressor {
        on: true,
        threshold: -30.0,
        ratio: 10.0,
        attack: 0.1,
        release: 50.0,
        key: 2,
    };
    // Another pedal keeps this track in the prepared mix while the
    // compressor switches between its own input and the track.
    chain.limiter.on = true;
    chain.limiter.key = 2;
    let mut board = Box::new(Board::new());
    board.apply(&chain);
    board.settle();
    for _ in 0..4000 {
        board.process(
            0.001,
            0.001,
            [None, Some((0.8, 0.8)), None, Some((0.0, 0.0))],
        );
    }
    chain.bus_compressor.key = 0;
    board.apply(&chain);
    for _ in 0..50_000 {
        board.process(0.001, 0.001, [None, None, None, Some((0.0, 0.0))]);
    }
    let mut own = board.clone();
    chain.bus_compressor.key = 2;
    board.apply(&chain);
    let mut difference = 0.0_f32;
    for _ in 0..2000 {
        let keyed = board.process(
            0.001,
            0.001,
            [None, Some((0.0, 0.0)), None, Some((0.0, 0.0))],
        );
        let expected = own.process(0.001, 0.001, [None, None, None, Some((0.0, 0.0))]);
        difference = difference.max((keyed.0 - expected.0).abs());
    }
    assert!(
        difference < 1e-7,
        "The old key changed the output by {difference}"
    );
}

#[test]
fn switching_and_moving_pedals_while_playing_does_not_click() {
    // A low tone moves at most 0.007 per frame. Switching the limiter
    // on with 12 dB of gain changes its level and delays it, and moving
    // the tube changes the sound again; without a crossfade either
    // would jump.
    let tone: Vec<f32> = (0..44_100)
        .flat_map(|i| {
            let v =
                (0.5 * (2.0 * std::f64::consts::PI * 100.0 * i as f64 / dsp::RATE).sin()) as f32;
            [v, v]
        })
        .collect();
    let mut chain = MasterChain::default();
    chain.tube.on = true;
    chain.tube.drive = 10.0;
    let control = MasterControl::new(&chain);
    let mut processor = MasterProcessor::live(control.clone());
    let mut output = Vec::new();
    for (index, block) in tone.chunks(1024).enumerate() {
        if index == 10 {
            chain.limiter.on = true;
            chain.limiter.gain = 12.0;
            control.set(&chain);
        }
        if index == 20 {
            let from = chain
                .order
                .iter()
                .position(|kind| *kind == PedalKind::Tube)
                .unwrap();
            chain.move_pedal(from, 11);
            control.set(&chain);
        }
        let mut block = block.to_vec();
        processor.process(&mut block);
        output.extend(block);
    }
    let steps = output
        .chunks(2)
        .zip(output.chunks(2).skip(1))
        .map(|(a, b)| (b[0] - a[0]).abs())
        .fold(0.0_f32, f32::max);
    assert!(steps < 0.03, "A step of {steps}");
    assert_eq!(processor.latency(), LIVE_LATENCY);
}

#[test]
fn playback_measures_the_input_and_output() {
    let program = dsp::program(3 * 44_100);
    let mut chain = mastered();
    chain.trim = -2.0;
    let control = MasterControl::new(&chain);
    assert_eq!(control.meters().run, 0);
    let mut processor = MasterProcessor::live(control.clone());
    let mut audio = interleaved(&program);
    for block in audio.chunks_mut(2048) {
        processor.process(block);
    }
    let reading = control.meters();
    assert_eq!(reading.run, 1);
    // The program sits near -18 dBFS RMS, 0 VU, before the trim.
    assert!((-6.0..1.0).contains(&reading.input_vu), "{reading:?}");
    assert!(reading.momentary.is_finite() && reading.integrated.is_finite());
    assert!(
        reading.true_peak_max <= chain.limiter.ceiling + 1e-3,
        "{reading:?}"
    );
    assert!(reading.compressor_reduction > 0.0 || reading.limiter_reduction > 0.0);
    // Export measures nothing.
    let mut exported = interleaved(&program);
    MasterProcessor::new(&chain).process(&mut exported);
    assert_eq!(control.meters().run, 1);
}

#[test]
fn mastering_pedals_at_their_defaults_raise_loudness_within_the_ceiling() {
    let program = dsp::program(3 * 44_100);
    let measure = |chain: &MasterChain| {
        let control = MasterControl::new(chain);
        let mut processor = MasterProcessor::live(control.clone());
        let mut audio = interleaved(&program);
        for block in audio.chunks_mut(1024) {
            processor.process(block);
        }
        control.meters()
    };
    let dry = measure(&MasterChain::default());
    let wet = measure(&mastered());
    let input_power = program
        .iter()
        .map(|(l, r)| (f64::from(*l).powi(2) + f64::from(*r).powi(2)) / 2.0)
        .sum::<f64>()
        / program.len() as f64;
    let input_db = 10.0 * input_power.log10();
    println!(
        "Input RMS {input_db:.4} dBFS; dry {:.4} LUFS; wet {:.4} LUFS; true peak {:.4} dBTP",
        dry.integrated, wet.integrated, wet.true_peak_max
    );
    assert!((input_db + 18.0).abs() < 0.01);
    assert!((wet.integrated + 11.0).abs() <= 1.0, "{wet:?}");
    assert!(wet.integrated > dry.integrated + 3.0, "{dry:?} {wet:?}");
    assert!(wet.true_peak_max <= -1.0 + 1e-3, "{wet:?}");
}

#[test]
fn knob_gestures_name_the_one_knob_that_moved() {
    let chain = MasterChain::default();
    let mut moved = chain;
    Knob::CompressorAttack.set(&mut moved, 0.3);
    assert_eq!(chain.changed_knob(&moved), Some(Knob::CompressorAttack));
    Knob::BoosterBand(4).set(&mut moved, 70.4);
    assert_eq!(moved.booster_eq.bands[4], 70);
    assert_eq!(chain.changed_knob(&moved), None);
    let mut switched = chain;
    switched.tape.speed = 15;
    assert_eq!(chain.changed_knob(&switched), None);
    Knob::Trim.set(&mut switched, 99.0);
    assert_eq!(switched.trim, 24.0);
    assert_eq!(Knob::LiftAir.pedal(), Some(PedalKind::LiftEq));
    assert_eq!(Knob::Trim.pedal(), None);
    assert_eq!(Knob::all().count(), 41);
    // Every default sits inside its range, on its step.
    for knob in Knob::all() {
        let range = knob.range();
        let value = knob.get(&chain);
        assert_eq!(value, range.default, "{knob:?}");
        let steps = (value - range.low) / range.step;
        assert!((steps - steps.round()).abs() < 1e-3, "{knob:?}");
    }
}

#[test]
fn settings_are_validated() {
    let mut chain = MasterChain::default();
    assert!(chain.validate().is_ok());
    chain.order[1] = PedalKind::BoosterEq;
    assert!(chain.validate().is_err());
    chain.order = PedalKind::ALL;
    chain.booster_eq.bands[3] = 101;
    assert!(chain.validate().is_err());
    chain.booster_eq.bands[3] = 100;
    chain.booster_compressor.speed = 1001;
    assert!(chain.validate().is_err());
    chain.booster_compressor.speed = 1000;
    assert!(chain.validate().is_ok());
    for (knob, value) in [
        (Knob::Trim, 24.5),
        (Knob::CompressorRatio, 1.0),
        (Knob::LimiterCeiling, 0.1),
        (Knob::TubeMix, f32::NAN),
        (Knob::ImagingWidth, f32::INFINITY),
    ] {
        let mut changed = chain;
        match knob {
            Knob::Trim => changed.trim = value,
            Knob::CompressorRatio => changed.bus_compressor.ratio = value,
            Knob::LimiterCeiling => changed.limiter.ceiling = value,
            Knob::TubeMix => changed.tube.mix = value,
            _ => changed.imaging.width = value,
        }
        assert!(changed.validate().is_err(), "{knob:?}");
    }
    chain.tape.speed = 7;
    assert!(chain.validate().is_err());
    chain.tape.speed = 30;
    chain.order.swap(11, 12);
    assert!(
        chain
            .validate()
            .unwrap_err()
            .contains("limiter must be the last")
    );
}

#[test]
fn the_limiter_stays_last() {
    let mut chain = MasterChain::default();
    assert!(!PedalKind::Limiter.movable() && PedalKind::Tape.movable());
    chain.move_pedal(12, 0);
    chain.move_pedal(0, 12);
    assert_eq!(
        chain.order,
        PedalKind::ALL,
        "No move reaches the limiter's place."
    );
    chain.move_pedal(0, 11);
    assert_eq!(
        chain.order[11..],
        [PedalKind::BoosterEq, PedalKind::Limiter]
    );
}
