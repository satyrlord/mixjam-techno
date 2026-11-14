//! The Time Stretcher worker and its results.
use super::*;
use std::sync::mpsc;

fn wait_for_stretch(session: &mut Session) {
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
    while session.is_stretching() {
        let delay = session.poll();
        if session.is_stretching() {
            assert!(delay.is_some_and(|delay| delay <= 20));
        }
        assert!(
            std::time::Instant::now() < deadline,
            "stretch worker stalled"
        );
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
}
fn finish_stretch(session: &mut Session) -> (StretchSettings, Result<Pcm, String>) {
    wait_for_stretch(session);
    session.take_stretch_result().expect("stretch result")
}
#[test]
fn stretch_worker_preserves_source_and_settings_and_requires_result_consumption() {
    let (mut session, root) = conflict_fixture();
    let settings = StretchSettings {
        source_id: 1_000_000,
        source_bpm: 160.0,
        target_bpm: 120.0,
    };
    session.start_stretch(settings).unwrap();
    assert!(session.is_stretching());
    assert!(session.start_stretch(settings).is_err());
    assert!(session.take_stretch_result().is_none());
    // Source additions use copy-on-write. A worker keeps its original
    // source even if the same ID is registered with another path later.
    let mut replacement = session.project().imported_samples[0].clone();
    replacement.path = session.sample_conflicts()[0].candidates[1].clone();
    Arc::make_mut(&mut session.library)
        .apply_imports(&[replacement])
        .unwrap();
    assert!(
        session
            .source_pcm(Some(settings.source_id))
            .unwrap()
            .samples[0]
            < 0.0
    );
    let (captured, result) = finish_stretch(&mut session);
    assert_eq!(captured, settings);
    let pcm = result.unwrap();
    assert_eq!(pcm.frames(), 29_400);
    assert!(pcm.samples.iter().any(|sample| *sample > 0.0));
    assert!(pcm.samples.iter().all(|sample| *sample >= 0.0));
    assert!(session.take_stretch_result().is_none());
    assert!(!session.is_previewing());
    session.start_stretch(settings).unwrap();
    wait_for_stretch(&mut session);
    assert!(session.start_stretch(settings).is_err());
    assert!(session.take_stretch_result().unwrap().1.is_ok());
    std::fs::remove_dir_all(root).unwrap();
}
#[test]
fn stretch_worker_errors_and_disconnects_leave_a_consumable_result() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let settings = StretchSettings {
        source_id: 99,
        source_bpm: 140.0,
        target_bpm: 120.0,
    };
    session.start_stretch(settings).unwrap();
    let (captured, result) = finish_stretch(&mut session);
    assert_eq!(captured, settings);
    assert!(result.unwrap_err().contains("not in the library"));
    let (sender, receiver) = mpsc::channel();
    session.stretch_job = Some(StretchJob { settings, receiver });
    drop(sender);
    let (captured, result) = finish_stretch(&mut session);
    assert_eq!(captured, settings);
    assert!(result.unwrap_err().contains("stopped unexpectedly"));
    assert!(!session.is_stretching());
    assert!(session.take_stretch_result().is_none());
}
#[test]
fn stretch_results_are_cancelled_when_the_library_or_song_changes() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    let settings = StretchSettings {
        source_id: 99,
        source_bpm: 140.0,
        target_bpm: 120.0,
    };
    let (sender, receiver) = mpsc::channel();
    session.stretch_job = Some(StretchJob { settings, receiver });
    session.set_library(Library::empty());
    assert!(!session.is_stretching());
    let (captured, result) = session.take_stretch_result().unwrap();
    assert_eq!(captured, settings);
    assert!(result.is_err());
    assert!(sender.send(Err("late worker result".into())).is_err());
    session.stretch_result = Some((
        settings,
        Ok(Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 16],
        }),
    ));
    session.new_mix().unwrap();
    assert!(session.take_stretch_result().unwrap().1.is_err());
    assert!(!session.is_previewing());
}
#[test]
fn stretch_preview_status_uses_the_actual_player_and_transport_revision() {
    let mut session = Session::headless(Arc::new(Library::empty()), Project::default()).unwrap();
    assert!(!session.is_previewing());
    let initial_revision = session.preview_revision();
    session.play_pcm(
        Pcm {
            sample_rate: 44_100,
            channels: 1,
            samples: vec![0.25; 16],
        },
        "Preview",
    );
    assert!(!session.is_previewing());
    assert_eq!(session.preview_revision(), initial_revision);
    // A stale logical preview flag alone must not keep an indicator on.
    session.previewing = true;
    assert!(!session.is_previewing());
    assert_eq!(session.poll(), None);
    session.stop();
    assert_ne!(session.preview_revision(), initial_revision);
    let stopped_revision = session.preview_revision();
    session.pause();
    assert_ne!(session.preview_revision(), stopped_revision);
}
