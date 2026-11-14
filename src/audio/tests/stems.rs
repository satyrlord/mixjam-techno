use super::*;

/// A failed stem render must keep every earlier export, including files
/// whose new samples were staged before the failure.
fn rejected_stems_keep_saved_files(project: &Project, mix: &Mix, part: &str) {
    let folder = std::env::temp_dir().join(format!(
        "mixjam-stems-rejected-{}-{}",
        std::process::id(),
        std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos()
    ));
    std::fs::create_dir_all(&folder).unwrap();
    let names: Vec<String> = mix
        .sounding_tracks()
        .iter()
        .map(|track| format!("{} - Track {:02}.wav", project.title, track + 1))
        .chain(
            mix.returning_buses(&project.fx)
                .iter()
                .map(|bus| format!("{} - FX {} return.wav", project.title, bus + 1)),
        )
        .collect();
    for name in &names {
        std::fs::write(folder.join(name), name.as_bytes()).unwrap();
    }
    let result = mix.write_stems(&folder, &project.title, &project.fx);
    let preserved = names
        .iter()
        .all(|name| std::fs::read(folder.join(name)).unwrap() == name.as_bytes());
    let entries = std::fs::read_dir(&folder).unwrap().count();
    std::fs::remove_dir_all(&folder).unwrap();
    let error = result.expect_err("An over-range part must not become a clipped stem.");
    assert!(error.contains(part), "{error}");
    assert!(error.contains("Lower"), "{error}");
    assert!(preserved, "Every saved stem must remain unchanged.");
    assert_eq!(entries, names.len(), "No staging files may remain.");
}

#[test]
fn over_range_track_stems_reject_even_when_the_mix_does_not_clip() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    editor.place(1, 1, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.master_gain = 1.0;
    project.tracks[0].polarity_inverted = true;
    project.tracks[1].gain = 2.0;
    let samples = HashMap::from([(
        1,
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.75; 4410],
        },
    )]);
    let mix = mix_with_samples(&project, &samples).unwrap();
    let whole = mix.to_pcm(&project.master, &project.fx).unwrap();
    assert_eq!(&whole.samples[..2], &[0.75, 0.75]);
    rejected_stems_keep_saved_files(&project, &mix, "Track 2");
}

#[test]
fn over_range_fx_return_stems_reject_and_keep_saved_files() {
    let mut editor = crate::engine::Editor::new(Project::default());
    editor.place(1, 0, 0, 256).unwrap();
    let mut project = editor.project().clone();
    project.tracks[0].sends[2] = 1.0;
    project.fx.buses[2].limiter = false;
    project.fx.buses[2].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
        mode: crate::fx::TimeMode::Free,
        time_ms_l: 20.0,
        time_ms_r: 20.0,
        feedback: 0.0,
        ping_pong: false,
        width: 100.0,
        low_cut: 20.0,
        high_cut: 20_000.0,
        mod_depth: 0.0,
        character: crate::fx::DelayCharacter::Digital,
        duck_amount: 0.0,
        output_db: 12.0,
        ..Default::default()
    });
    let mix = mix_with_samples(&project, &HashMap::from([(1, tone(1000.0))])).unwrap();
    rejected_stems_keep_saved_files(&project, &mix, "FX 3 return");
}

#[test]
fn exported_tracks_and_returns_sum_to_the_mix_without_the_board() {
    let mut editor = crate::engine::Editor::new(Project::default());
    for (sample, track) in [(1, 0), (2, 2), (3, 4), (1, 6)] {
        editor.place(sample, track, 0, 256).unwrap();
    }
    let mut project = editor.project().clone();
    project.title = "Reality: take 2".into();
    let samples = HashMap::from([(1, tone(110.0)), (2, tone(3000.0)), (3, tone(700.0))]);
    project.master_gain = 0.5;
    project.tracks[0].gain = 0.6;
    project.tracks[0].pan = -0.4;
    project.tracks[0].sends[1] = 0.5;
    project.tracks[2].eq = crate::eq::ChannelEq {
        on: true,
        high_db: -6.0,
        ..Default::default()
    };
    project.tracks[2].sends[1] = 0.3;
    project.tracks[4].automation.volume = vec![
        crate::automation::Point {
            tick: 0,
            value: -20.0,
        },
        crate::automation::Point {
            tick: 256,
            value: 0.0,
        },
    ];
    project.tracks[4].right_enabled = false;
    project.tracks[4].sends[3] = 0.4;
    // FX 4's return lane rises through the song; the parts follow it.
    project.return_automation[3] = vec![
        crate::automation::Point {
            tick: 0,
            value: 0.2,
        },
        crate::automation::Point {
            tick: 256,
            value: 1.0,
        },
    ];
    // A muted track and an unpowered bus have no file.
    project.tracks[6].muted = true;
    project.fx.buses[1].module =
        crate::fx::Module::AetherformReverb(crate::fx::Aetherform::preset("Dark Hall").unwrap());
    project.fx.buses[3].module = crate::fx::Module::EchoformDelay(crate::fx::Echoform {
        mode: crate::fx::TimeMode::Free,
        time_ms_l: 300.0,
        time_ms_r: 300.0,
        feedback: 30.0,
        ..crate::fx::Echoform::default()
    });
    project.fx.buses[2].module = project.fx.buses[3].module;
    project.fx.buses[2].powered = false;
    // The board is left out of the parts, whatever it holds.
    project.master.trim = 6.0;
    project.master.limiter.on = true;

    let mix = mix_with_samples(&project, &samples).unwrap();
    let folder = std::env::temp_dir().join(format!("mixjam-stems-{}", std::process::id()));
    std::fs::create_dir_all(&folder).unwrap();
    let written = mix
        .write_stems(&folder, &project.title, &project.fx)
        .unwrap();
    assert_eq!(
        written,
        Stems {
            tracks: 3,
            returns: 2
        }
    );
    let mut names: Vec<String> = std::fs::read_dir(&folder)
        .unwrap()
        .map(|entry| entry.unwrap().file_name().to_string_lossy().into_owned())
        .collect();
    names.sort();
    assert_eq!(
        names,
        [
            "Reality_ take 2 - FX 2 return.wav",
            "Reality_ take 2 - FX 4 return.wav",
            "Reality_ take 2 - Track 01.wav",
            "Reality_ take 2 - Track 03.wav",
            "Reality_ take 2 - Track 05.wav",
        ]
    );
    let parts: Vec<Pcm> = names
        .iter()
        .map(|name| read_wav(&folder.join(name)).unwrap())
        .collect();
    let whole = folder.join("mix.wav");
    let bypassed = MasterChain {
        bypass: true,
        ..project.master
    };
    mix.write_wav(&whole, &bypassed, &project.fx).unwrap();
    let whole = read_wav(&whole).unwrap();
    std::fs::remove_dir_all(&folder).unwrap();
    assert!(whole.frames() > mix.frames() + 44_100, "the tails ring on");
    for part in &parts {
        assert_eq!(part.samples.len(), whole.samples.len());
        assert!(part.samples.iter().any(|sample| sample.abs() > 0.01));
    }
    // Each file rounds to 16 bits once; so does the mix.
    let step = 1.0 / 32768.0;
    let worst = (0..whole.samples.len())
        .map(|at| {
            let sum: f32 = parts.iter().map(|part| part.samples[at]).sum();
            (sum - whole.samples[at]).abs()
        })
        .fold(0.0_f32, f32::max);
    assert!(
        worst <= (parts.len() + 1) as f32 * step,
        "the parts differ from the mix by {} steps",
        worst / step
    );
    // Track 5 plays on the left only.
    let track5 = &parts[4];
    assert!(track5.samples.iter().skip(1).step_by(2).all(|s| *s == 0.0));
}
