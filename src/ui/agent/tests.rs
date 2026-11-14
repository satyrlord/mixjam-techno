use super::request::clip_edit;
use super::*;
use mixjam_techno::application::FadeCurve;

/// A `clip` request for clip 4 with the given gain, length, and fade in.
fn change(
    gain_db: Option<f32>,
    length_ticks: Option<u32>,
    fade_in: Option<(u32, FadeCurve)>,
) -> Request {
    Request::Clip {
        clip: 4,
        gain_db,
        offset_frames: None,
        length_ticks,
        fade_in_ticks: fade_in.map(|(ticks, _)| ticks),
        fade_in_curve: fade_in.map(|(_, curve)| curve),
        fade_out_ticks: None,
        fade_out_curve: None,
        pitch_cents: None,
        reverse: None,
    }
}

#[test]
fn a_clip_request_changes_only_the_fields_it_gives() {
    let mut clip = Clip {
        id: 4,
        sample_id: 1,
        track: 2,
        start_tick: 512,
        length_ticks: 256,
        offset_frames: 10,
        gain: 0.5,
        fades: Default::default(),
        pitch_cents: -100,
        reverse: false,
    };
    clip.fades.fade_out.ticks = 64;
    let EditCommand::SetClip {
        id,
        start_tick,
        gain,
        offset_frames,
        length_ticks,
        fades,
        pitch_cents,
        reverse,
    } = clip_edit(
        &clip,
        change(Some(0.0), None, Some((32, FadeCurve::Exponential))),
    )
    .unwrap()
    else {
        panic!("A clip request makes a clip edit.")
    };
    assert_eq!((id, start_tick, gain), (4, 512, 1.0));
    assert_eq!((offset_frames, length_ticks, pitch_cents), (10, 256, -100));
    assert!(!reverse);
    assert_eq!(fades.fade_in.ticks, 32);
    assert_eq!(fades.fade_in.curve, FadeCurve::Exponential);
    assert_eq!(fades.fade_out, clip.fades.fade_out);

    let silent = clip_edit(&clip, change(Some(-60.0), None, None)).unwrap();
    assert!(matches!(silent, EditCommand::SetClip { gain: 0.0, .. }));
    for invalid in [
        change(Some(-60.1), None, None),
        change(Some(7.0), None, None),
        change(None, Some(0), None),
        change(None, None, Some((200, FadeCurve::Linear))),
    ] {
        assert!(clip_edit(&clip, invalid.clone()).is_err(), "{invalid:?}");
    }
    // A shorter clip shortens a fade that the request leaves out.
    assert!(clip_edit(&clip, change(None, Some(32), None)).is_ok());
    // Reverse switches only when the request gives it.
    let Request::Clip { .. } = change(None, None, None) else {
        unreachable!()
    };
    let mut flip = change(None, None, None);
    if let Request::Clip { reverse, .. } = &mut flip {
        *reverse = Some(true);
    }
    let reversed = clip_edit(&clip, flip).unwrap();
    assert!(matches!(
        reversed,
        EditCommand::SetClip {
            reverse: true,
            gain: 0.5,
            ..
        }
    ));
    clip.reverse = true;
    let kept = clip_edit(&clip, change(Some(0.0), None, None)).unwrap();
    assert!(matches!(kept, EditCommand::SetClip { reverse: true, .. }));
}
