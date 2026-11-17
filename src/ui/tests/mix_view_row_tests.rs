//! A Mix view track row on a bare context: plain values in, an outcome out,
//! with no app and no session, so the row's look can change without a song.

use super::*;
use mix_view::rows::{Outcome, RowFacts};

/// The row's rectangle: wide enough for every column, so none scrolls.
fn row() -> Rect {
    Rect::from_min_size(Pos2::new(0.0, 40.0), Vec2::new(1400.0, 32.0))
}

/// A bare context with the app's style and accessibility on.
fn context() -> egui::Context {
    let ctx = egui::Context::default();
    ctx.enable_accesskit();
    configure_style(&ctx);
    ctx
}

/// Paints Track 3's row with `mix` and `facts` after `events`, and returns
/// the frame and what the row reported.
fn paint(
    ctx: &egui::Context,
    facts: RowFacts,
    mix: &mut TrackMix,
    events: Vec<egui::Event>,
) -> (egui::FullOutput, Outcome) {
    let mut outcome = None;
    let output = ctx.run(
        egui::RawInput {
            screen_rect: Some(Rect::from_min_size(Pos2::ZERO, Vec2::new(1440.0, 120.0))),
            events,
            ..Default::default()
        },
        |ctx| {
            egui::CentralPanel::default().show(ctx, |ui| {
                outcome = Some(mix_view::track_row(
                    ui,
                    row(),
                    0.0,
                    2,
                    ("good grasp", "good grasp\nsecond line"),
                    &facts,
                    mix,
                ));
            });
        },
    );
    (output, outcome.expect("The row paints."))
}

/// Clicks the control named `name`, and returns what the row reported on
/// the release.
fn click_control(ctx: &egui::Context, facts: RowFacts, mix: &mut TrackMix, name: &str) -> Outcome {
    let (output, _) = paint(ctx, facts, mix, vec![]);
    let pos = screen(&output).node(name).rect().center();
    let button = |pressed| egui::Event::PointerButton {
        pos,
        button: egui::PointerButton::Primary,
        pressed,
        modifiers: egui::Modifiers::NONE,
    };
    paint(
        ctx,
        facts,
        mix,
        vec![egui::Event::PointerMoved(pos), button(true)],
    );
    paint(ctx, facts, mix, vec![button(false)]).1
}

fn plain() -> TrackMix {
    Track::default().mix()
}

#[test]
fn a_plain_row_names_every_control_and_paints_only_its_sounds() {
    each_theme(|theme| {
        let ctx = context();
        let mut mix = plain();
        paint(&ctx, RowFacts::default(), &mut mix, vec![]);
        let (output, outcome) = paint(&ctx, RowFacts::default(), &mut mix, vec![]);
        let view = screen(&output);
        for suffix in [
            "mute",
            "solo",
            "left output",
            "right output",
            "polarity invert",
            "gain",
            "gain value",
            "pan",
            "EQ",
            "send FX 1",
            "send FX 4",
        ] {
            view.node(&format!("Track 3 {suffix}"));
        }
        view.node("Reset Track 3 gain to unity");
        view.node("Center Track 3 pan");
        assert_eq!(
            view.text("good grasp").color(),
            roles().strip_text,
            "{theme:?}"
        );
        for quiet in ["Unity", "Center", "Muted", "No output", "0"] {
            assert!(
                !view.shows(quiet),
                "{theme:?}: {quiet} shows on a plain row."
            );
        }
        assert!(!outcome.changed && !outcome.open_eq);
        assert_eq!(mix, plain(), "Painting edits nothing.");
    });
}

#[test]
fn a_silent_row_dims_all_but_its_own_cause() {
    let ctx = context();
    let mut muted = TrackMix {
        muted: true,
        ..plain()
    };
    paint(&ctx, RowFacts::default(), &mut muted, vec![]);
    let (output, _) = paint(&ctx, RowFacts::default(), &mut muted, vec![]);
    let view = screen(&output);
    let dimmed = |text: Text| (text.opacity() - mix_view::rows::SILENT_OPACITY).abs() < 0.01;
    assert_eq!(view.text("Muted").opacity(), 1.0);
    assert!(dimmed(view.text("good grasp")));
    // Another track's solo dims the row but names no cause on it.
    let soloed = RowFacts {
        any_solo: true,
        ..RowFacts::default()
    };
    let mut quiet = plain();
    let (output, _) = paint(&ctx, soloed, &mut quiet, vec![]);
    let view = screen(&output);
    assert!(!view.shows("Solo elsewhere"));
    assert!(dimmed(view.text("good grasp")));
    // A plain row is not dimmed.
    let (output, _) = paint(&ctx, RowFacts::default(), &mut quiet, vec![]);
    assert_eq!(screen(&output).text("good grasp").opacity(), 1.0);
}

#[test]
fn a_moved_row_names_its_pan_and_used_sends() {
    let ctx = context();
    let mut mix = TrackMix {
        pan: -0.25,
        sends: [0.0, 0.35, 0.0, 0.0],
        ..plain()
    };
    paint(&ctx, RowFacts::default(), &mut mix, vec![]);
    let (output, _) = paint(&ctx, RowFacts::default(), &mut mix, vec![]);
    let view = screen(&output);
    for shown in ["25% left", "35"] {
        assert!(view.shows(shown), "{shown} is missing.");
    }
    assert!(!view.shows("0"), "An unused send shows no amount.");
    let automated = RowFacts {
        pan_automated: true,
        ..RowFacts::default()
    };
    let mut centred = plain();
    let (output, _) = paint(&ctx, automated, &mut centred, vec![]);
    assert!(screen(&output).shows("Pan automated"));
}

#[test]
fn a_row_edits_only_its_plain_mix_and_reports_each_change() {
    let ctx = context();
    let facts = RowFacts::default();
    let mut mix = plain();
    let outcome = click_control(&ctx, facts, &mut mix, "Track 3 mute");
    assert!(outcome.changed);
    assert_eq!(
        mix,
        TrackMix {
            muted: true,
            ..plain()
        }
    );
    let outcome = click_control(&ctx, facts, &mut mix, "Track 3 left output");
    assert!(outcome.changed && !mix.left_enabled);
    // Center reports a change only when the pan moved.
    mix.pan = 0.3;
    assert!(click_control(&ctx, facts, &mut mix, "Center Track 3 pan").changed);
    assert_eq!(mix.pan, 0.0);
    assert!(!click_control(&ctx, facts, &mut mix, "Center Track 3 pan").changed);
}

#[test]
fn eq_opens_beside_the_row_without_editing_the_mix() {
    let ctx = context();
    let mut mix = plain();
    let outcome = click_control(&ctx, RowFacts::default(), &mut mix, "Track 3 EQ");
    assert!(outcome.open_eq && !outcome.changed);
    assert_eq!(mix, plain());
    let anchor = outcome.eq_anchor.expect("The row anchors its EQ window.");
    assert_eq!(anchor.left(), row().left());
    assert_eq!(anchor.y_range(), row().y_range());
    assert!(anchor.right() < row().right());
}
