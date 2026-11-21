//! The Inspector's clip pages: one clip, and several clips together.
use super::super::super::controls::{NumberField, number_field};
use super::super::super::view::TICKS_PER_BEAT;
use super::cards::{Fields, Sound, card, row, row_label, sound_cards, values};
use super::*;
use mixjam_techno::application::{ClipSound, MAX_TICKS};

impl MixJamApp {
    pub(in super::super) fn clip_page(&mut self, ui: &mut egui::Ui, id: u64) {
        let project = self.session.project();
        let Some(clip) = project.clips.iter().find(|clip| clip.id == id).cloned() else {
            return;
        };
        let sample = self.session.library().sample_by_id(clip.sample_id).cloned();
        let place = format!(
            "{}  ·  Track {}",
            bars_text(clip.start_tick, clip.start_tick + clip.length_ticks),
            clip.track + 1
        );
        let Some(sample) = sample else {
            let name = self
                .session
                .missing_sample(clip.sample_id)
                .map(|missing| missing.name.clone());
            title(
                ui,
                name.as_deref().unwrap_or("Missing sound"),
                &["Missing sound", &place],
            );
            note(
                ui,
                "The sound's file is not in the libraries, so the clip plays silence. Locate \
                 the file to hear it again; every clip of the sound follows.",
            );
            let response = ui.button("Locate the sound…");
            if name_control(response, &format!("{CLIP} locate sound"))
                .hover_hint(
                    "Choose the sound's audio file. Import details in the status bar list \
                     every missing sound.",
                )
                .clicked()
            {
                self.resolve_missing_sample(clip.sample_id);
            }
            return;
        };
        title(ui, &sample.name, &[group_label(&sample.group_name), &place]);
        let bpm = self
            .session
            .project()
            .sample_playback_bpm(sample.id, sample.source_bpm);
        let mut next = clip.clone();
        self.clip_picture(ui, &clip, &sample, bpm, &mut next.fades);

        let mut sound = Sound::of(&next);
        let rooms = [
            clip.length_ticks.saturating_sub(next.fades.fade_out.ticks),
            clip.length_ticks.saturating_sub(next.fades.fade_in.ticks),
        ];
        let (touched, mut discrete) = sound_cards(ui, &mut sound, Fields::default(), rooms);
        next.gain = sound.gain;
        next.fades = sound.fades;
        next.pitch_cents = sound.pitch(clip.pitch_cents, touched);
        next.reverse = sound.reverse;

        section_heading(ui, "Position");
        let rows = card(ui, 3);
        let at = row(rows, 0);
        row_label(ui, at, "Start");
        let mut bar = f64::from(next.start_tick) / f64::from(TICKS_PER_BAR) + 1.0;
        if number_field(
            ui,
            values(at),
            NumberField {
                name: &format!("{CLIP} start (bar)"),
                range: 1.0..=(f64::from(MAX_TICKS - next.length_ticks) / f64::from(TICKS_PER_BAR)
                    + 1.0),
                speed: 1.0 / 16.0,
                affixes: ("bar ", ""),
                mixed: false,
                hint: "The bar the clip starts on; 1.5 is halfway through bar 1. Its lane stays \
                       the same.",
            },
            &mut bar,
            &mut discrete,
        ) {
            next.start_tick = ((bar - 1.0) * f64::from(TICKS_PER_BAR)).round().max(0.0) as u32;
        }
        let at = row(rows, 1);
        row_label(ui, at, "Length");
        let mut bars = f64::from(next.length_ticks) / f64::from(TICKS_PER_BAR);
        let unit = if bars == 1.0 { " bar" } else { " bars" };
        if number_field(
            ui,
            values(at),
            NumberField {
                name: &format!("{CLIP} length (bars)"),
                range: 1.0 / f64::from(TICKS_PER_BAR)
                    ..=f64::from(MAX_TICKS - next.start_tick) / f64::from(TICKS_PER_BAR),
                speed: 1.0 / 16.0,
                affixes: ("", unit),
                mixed: false,
                hint: "The clip's length in bars. A clip longer than its sound plays silence \
                       after it.",
            },
            &mut bars,
            &mut discrete,
        ) {
            next.length_ticks = (bars * f64::from(TICKS_PER_BAR)).round().max(1.0) as u32;
            next.fades = next.fades.fit(next.length_ticks);
        }
        let at = row(rows, 2);
        row_label(ui, at, "Trim start");
        // The trim is a count of source frames; beats at the song's tempo
        // read like the clip's other times.
        let frames_per_beat = view::frames_per_tick(&sample, bpm) * f64::from(TICKS_PER_BEAT);
        if frames_per_beat > 0.0 {
            let mut beats = f64::from(next.offset_frames) / frames_per_beat;
            let last = f64::from(sample.frame_count.saturating_sub(1)) / frames_per_beat;
            let unit = if beats == 1.0 { " beat" } else { " beats" };
            if number_field(
                ui,
                values(at),
                NumberField {
                    name: &format!("{CLIP} trim start (beats)"),
                    range: 0.0..=last,
                    speed: 1.0 / 16.0,
                    affixes: ("", unit),
                    mixed: false,
                    hint: "How much of the sound's start the clip skips, in beats at the \
                           song's tempo. The clip stays where it is.",
                },
                &mut beats,
                &mut discrete,
            ) {
                next.offset_frames = ((beats * frames_per_beat).round().max(0.0) as u32)
                    .min(sample.frame_count.saturating_sub(1));
            }
        }

        if next != clip {
            let command = EditCommand::SetClip {
                id,
                start_tick: next.start_tick,
                gain: next.gain,
                offset_frames: next.offset_frames,
                length_ticks: next.length_ticks,
                fades: next.fades,
                pitch_cents: next.pitch_cents,
                reverse: next.reverse,
            };
            let result = if discrete {
                self.session.edit_discrete(command)
            } else {
                self.session.edit(command)
            };
            self.apply(result);
            ui.ctx().request_repaint();
        }
    }

    pub(in super::super) fn clips_page(&mut self, ui: &mut egui::Ui, count: usize) {
        let project = self.session.project();
        let clips: Vec<Clip> = project
            .clips
            .iter()
            .filter(|clip| self.selection.clips.contains(&clip.id))
            .cloned()
            .collect();
        let Some(first) = clips.first() else {
            return;
        };
        let start = clips.iter().map(|clip| clip.start_tick).min().unwrap_or(0);
        let end = clips
            .iter()
            .map(|clip| clip.start_tick + clip.length_ticks)
            .max()
            .unwrap_or(0);
        let tracks: BTreeSet<usize> = clips.iter().map(|clip| clip.track).collect();
        title(
            ui,
            &format!("{count} clips"),
            &[&format!(
                "{}  ·  {} {}",
                bars_text(start, end),
                tracks.len(),
                if tracks.len() == 1 { "track" } else { "tracks" }
            )],
        );
        let before = Sound::of(first);
        let differs =
            |value: &dyn Fn(&Clip) -> i64| clips.iter().any(|clip| value(clip) != value(first));
        let mixed = Fields {
            gain: clips.iter().any(|clip| clip.gain != first.gain),
            fade_in: differs(&|clip| i64::from(clip.fades.fade_in.ticks)),
            fade_in_curve: clips
                .iter()
                .any(|clip| clip.fades.fade_in.curve != first.fades.fade_in.curve),
            fade_out: differs(&|clip| i64::from(clip.fades.fade_out.ticks)),
            fade_out_curve: clips
                .iter()
                .any(|clip| clip.fades.fade_out.curve != first.fades.fade_out.curve),
            semitones: differs(&|clip| i64::from(clip.pitch_cents / 100)),
            cents: differs(&|clip| i64::from(clip.pitch_cents % 100)),
            reverse: clips.iter().any(|clip| clip.reverse != first.reverse),
        };
        // The fades fit each clip when they are set; the fields allow the
        // longest clip's length.
        let longest = clips
            .iter()
            .map(|clip| clip.length_ticks)
            .max()
            .unwrap_or(0);
        let mut sound = before;
        let (touched, discrete) = sound_cards(ui, &mut sound, mixed, [longest, longest]);
        if touched.any() {
            let sounds = clips
                .iter()
                .map(|clip| {
                    let mut fades = clip.fades;
                    if touched.fade_in {
                        fades.fade_in.ticks = sound.fades.fade_in.ticks;
                    }
                    if touched.fade_in_curve {
                        fades.fade_in.curve = sound.fades.fade_in.curve;
                    }
                    if touched.fade_out {
                        fades.fade_out.ticks = sound.fades.fade_out.ticks;
                    }
                    if touched.fade_out_curve {
                        fades.fade_out.curve = sound.fades.fade_out.curve;
                    }
                    ClipSound {
                        id: clip.id,
                        gain: if touched.gain { sound.gain } else { clip.gain },
                        fades,
                        pitch_cents: sound.pitch(clip.pitch_cents, touched),
                        reverse: if touched.reverse {
                            sound.reverse
                        } else {
                            clip.reverse
                        },
                    }
                })
                .collect();
            let command = EditCommand::SetClipSounds(sounds);
            let result = if discrete {
                self.session.edit_discrete(command)
            } else {
                self.session.edit(command)
            };
            self.apply(result);
            ui.ctx().request_repaint();
        }
        self.category_counts(ui, &clips);
        note(
            ui,
            "A change above sets every selected clip, as one undo step; a dash marks a value \
             that differs between them. Drag any clip to move them together; Delete removes \
             them all.",
        );
    }
}

/// "Bar 3" or "Bars 3 to 6" for a span of ticks.
fn bars_text(start: u32, end: u32) -> String {
    let (first, last) = bar_span(start, end);
    if first == last {
        format!("Bar {first}")
    } else {
        format!("Bars {first} to {last}")
    }
}
