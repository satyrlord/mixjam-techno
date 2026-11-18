//! The Help windows: the application's manual, which opens on a shortcut
//! sheet and lists its pages at the left, and each tool's page of it in the
//! tool's own window and palette, placed beside the tool.
use super::*;
use egui::accesskit::Role;

/// A page of Help.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(super) enum Topic {
    #[default]
    Shortcuts,
    Arrange,
    Automation,
    Recording,
    Mixer,
    MixView,
    Mastering,
    FxReturns,
    Workspace,
    Stretch,
    Effects,
    StepSynth,
    SampleEditor,
    AlbumReview,
}

impl Topic {
    pub(super) const ALL: [Topic; 14] = [
        Topic::Shortcuts,
        Topic::Arrange,
        Topic::Automation,
        Topic::Recording,
        Topic::Mixer,
        Topic::MixView,
        Topic::Mastering,
        Topic::FxReturns,
        Topic::Workspace,
        Topic::Stretch,
        Topic::Effects,
        Topic::StepSynth,
        Topic::SampleEditor,
        Topic::AlbumReview,
    ];

    pub(super) fn name(self) -> &'static str {
        match self {
            Topic::Shortcuts => "Shortcuts",
            Topic::Arrange => "Arrange",
            Topic::Automation => "Automation",
            Topic::Recording => "Recording",
            Topic::Mixer => "Mixer",
            Topic::MixView => "Mix view",
            Topic::Mastering => "Mastering",
            Topic::FxReturns => "FX returns",
            Topic::Workspace => "Workspace",
            Topic::Stretch => "Time Stretcher",
            Topic::Effects => "Effect Studio",
            Topic::StepSynth => "Step synth",
            Topic::SampleEditor => "Sample editor",
            Topic::AlbumReview => "Album Review",
        }
    }

    /// The page's own window, which the `?` of its tool or board opens.
    pub(super) fn window(self) -> Option<&'static str> {
        match self {
            Topic::Stretch => Some("Time Stretcher help"),
            Topic::Effects => Some("Effect Studio help"),
            Topic::StepSynth => Some("Step synth help"),
            Topic::SampleEditor => Some("Sample editor help"),
            Topic::Mastering => Some("Mastering help"),
            _ => None,
        }
    }

    /// The tool palette of the page, which its window and its mark take.
    pub(super) fn palette(self) -> Option<controls::Palette> {
        match self {
            Topic::Stretch | Topic::FxReturns => Some(controls::Palette::Marine),
            Topic::Effects => Some(controls::Palette::Red),
            Topic::StepSynth => Some(controls::Palette::Olive),
            Topic::SampleEditor => Some(controls::Palette::Granite),
            Topic::Mastering => Some(controls::Palette::Indigo),
            _ => None,
        }
    }

    /// The floating tool window that the page's window opens beside. Board
    /// pages open beside the lower panel instead.
    fn tool_window(self) -> Option<&'static str> {
        match self {
            Topic::Stretch => Some("Time Stretcher"),
            Topic::Effects => Some("Effect Studio"),
            Topic::SampleEditor => Some("Sample editor"),
            _ => None,
        }
    }

    fn blocks(self) -> &'static [Block] {
        match self {
            Topic::Shortcuts => SHORTCUTS,
            Topic::Arrange => ARRANGE,
            Topic::Automation => AUTOMATION,
            Topic::Recording => RECORDING,
            Topic::Mixer => MIXER,
            Topic::MixView => MIX_VIEW,
            Topic::Mastering => MASTERING,
            Topic::FxReturns => FX_RETURNS,
            Topic::Workspace => WORKSPACE,
            Topic::Stretch => STRETCH,
            Topic::Effects => EFFECTS,
            Topic::StepSynth => STEP_SYNTH,
            Topic::SampleEditor => SAMPLE_EDITOR,
            Topic::AlbumReview => ALBUM_REVIEW,
        }
    }

    /// Every line of the page, in order. Only the library's UI tests use it.
    #[cfg(test)]
    #[allow(dead_code)]
    pub(super) fn text(self) -> Vec<&'static str> {
        let mut lines = Vec::new();
        for block in self.blocks() {
            match block {
                Block::Heading(text) => lines.push(*text),
                Block::Facts(facts) | Block::Steps(facts) => lines.extend(facts.iter()),
                Block::Keys(rows) => {
                    for (action, keys, note) in *rows {
                        lines.push(*action);
                        lines.extend(keys.iter());
                        lines.push(*note);
                    }
                }
                Block::Terms(rows) => {
                    for (term, meaning) in *rows {
                        lines.push(*term);
                        lines.push(*meaning);
                    }
                }
            }
        }
        lines.retain(|line| !line.is_empty());
        lines
    }
}

/// One part of a page.
enum Block {
    Heading(&'static str),
    /// Short facts, one to a line.
    Facts(&'static [&'static str]),
    /// Numbered steps of one task.
    Steps(&'static [&'static str]),
    /// An action, the keys that do it, and a short note.
    Keys(&'static [(&'static str, &'static [&'static str], &'static str)]),
    /// A term and what it means.
    Terms(&'static [(&'static str, &'static str)]),
}
use Block::*;

/// The knob keys that every tool shares.
const KNOBS: Block = Keys(&[
    ("Adjust", &["Arrows"], "or drag, or turn the wheel"),
    (
        "Fine steps",
        &["Shift"],
        "while dragging or pressing an arrow",
    ),
    (
        "Exact value",
        &[],
        "Click the value, type, then press Enter",
    ),
    ("Default", &[], "Double-click the knob"),
]);

const SHORTCUTS: &[Block] = &[
    Heading("Song"),
    Keys(&[
        ("New song", &["Ctrl+N"], ""),
        ("Open", &["Ctrl+O"], "MIX, MIXJAM, or JSON"),
        ("Save", &["Ctrl+S"], "MIXJAM"),
        ("Save as", &["Ctrl+Shift+S"], "a new file"),
        ("Undo", &["Ctrl+Z"], ""),
        ("Redo", &["Ctrl+Shift+Z", "Ctrl+Y"], ""),
        ("All commands", &["Ctrl+K"], "everything the app can do"),
    ]),
    Heading("Playback"),
    Keys(&[
        ("Play / pause", &["Space"], ""),
        ("Seek", &[], "Click a bar number"),
        ("Loop", &[], "Drag across the bar ruler"),
        (
            "Audio capture",
            &["F12"],
            "saves the last 30 seconds of playback",
        ),
    ]),
    Heading("Clips"),
    Keys(&[
        (
            "Place sound",
            &[],
            "Drag it to a track, or select it and click a track",
        ),
        ("Preview", &[], "Double-click a sound or clip"),
        ("Move / copy", &["Alt"], "drag a clip; Alt copies it"),
        ("Fine snap", &["Ctrl"], "while dragging: sixteenth notes"),
        (
            "Select group",
            &["Shift"],
            "click clips, or drag an empty area",
        ),
        ("Select all", &["Ctrl+A"], ""),
        ("Duplicate", &["Ctrl+D"], "to the bars that follow"),
        ("Delete", &["Delete"], ""),
        ("Clip menu", &["Shift+F10"], "or right-click a clip"),
    ]),
    Heading("By keyboard"),
    Keys(&[
        ("Next control", &["Tab"], ""),
        (
            "Place sound",
            &["Enter"],
            "on a sound: the recording track, at the playhead",
        ),
        ("Cancel placing", &["Esc"], ""),
        ("Choose a clip", &["Arrows"], "in the lanes"),
        ("Move a clip", &["Alt+Arrows"], "a bar or a track"),
        ("Nudge a clip", &["Ctrl+Alt+Arrows"], "a sixteenth"),
        ("Preview clip", &["Enter"], ""),
        ("Adjust control", &["Arrows"], "on a fader, knob, or slider"),
        ("Switch", &["Space"], "M, S, L, and R"),
    ]),
    Heading("View"),
    Keys(&[
        (
            "Maximise",
            &["F"],
            "hides the side and lower panels; F restores them",
        ),
        ("Help", &["F1"], "opens and closes this window"),
    ]),
    Facts(&[
        "Ctrl shortcuts work from any control except a value being typed.",
        "Space plays when no control has focus, or when a dial, a fader, or the lanes have it.",
    ]),
];

const ARRANGE: &[Block] = &[
    Heading("Place sounds"),
    Facts(&[
        "Drag a sound to a track, or select a sound and click a track. A song has up to 64 tracks.",
        "An outline shows where the sound lands. Esc, or a click on a clip, ends click placement.",
        "By keyboard: Tab to a sound and press Enter. It lands on the recording track at the playhead.",
        "Double-click a sound or a clip to preview it.",
    ]),
    Heading("Edit clips"),
    Facts(&[
        "Drag a clip to move it; Alt-drag copies it. Hold Ctrl for sixteenth-note positions.",
        "Shift-click clips, or drag across an empty area, to select a group. Delete removes the selection.",
        "Ctrl+D copies the selected clips to the bars that follow.",
        "Right-click a clip, or press Shift+F10, for exact gain, trim, length, fades, and pitch, and for Preview, Duplicate, and Delete. A missing sound also offers Resolve manually.",
        "In the lanes, arrows choose a clip, Alt+arrows move it by a bar or a track, and Ctrl+Alt+arrows by a sixteenth. Enter previews it.",
    ]),
    Heading("Playback and loop"),
    Facts(&[
        "Space plays and pauses. Click a bar number to seek. Drag across the bar ruler to loop.",
        "Follow, beside Zoom, keeps the playhead in view. Turn it off to move through the song freely during playback.",
        "Drag Zoom, or turn the wheel over it, to zoom the timeline.",
    ]),
    Heading("Sections"),
    Facts(&[
        "Sections are named parts, such as Intro or Break. Each shows as a line across the lanes, with its name on the bar ruler, and lasts until the next one.",
    ]),
    Steps(&[
        "Double-click the bar ruler where the section starts.",
        "Type the name.",
        "Press Enter. Esc keeps the old name.",
    ]),
    Facts(&[
        "Double-click a name to rename it; right-click it to rename or delete it.",
        "With sections shown, Loop loops the section at the playhead. Sections, beside Follow, hides them.",
    ]),
    Heading("Boards and view"),
    Facts(&[
        "Mixer, Mastering, FX returns, and Step synth in the tool rail show their board in the lower panel. Choose it again to fold the panel. Drag the divider above the panel to resize it.",
        "Mix, in the score toolbar, turns the lane headers into mixer rows. The rail's Mixer returns the score to Arrange.",
        "F hides the tool rail, the Sounds panel, the Inspector, and the lower panel. F again restores them.",
    ]),
];

const AUTOMATION: &[Block] = &[
    Facts(&[
        "Choose a lane in the Automation menu above the arrangement. The lane shows on every track. Off returns to editing clips.",
    ]),
    Terms(&[
        (
            "Every track",
            "Volume (dB over the fader), Pan, Filter, Pitch, Repeat, and Repeat gate.",
        ),
        (
            "Sends",
            "FX 1 send to FX 4 send, each named after the effect on its bus.",
        ),
        (
            "Returns",
            "FX 1 return to FX 4 return: one line across the song.",
        ),
    ]),
    Heading("Points"),
    Keys(&[
        ("Add", &[], "Click the lane"),
        ("Move", &[], "Drag the point"),
        ("Delete", &[], "Double-click or right-click the point"),
        (
            "Single ticks",
            &["Ctrl"],
            "while dragging; points snap to 1/16 bar",
        ),
    ]),
    Facts(&[
        "Values move in a straight line between points. Repeat and Repeat gate move in steps.",
        "The menu marks the current lane, and shows how many tracks have points on each lane, or how many points a return lane has.",
        "A ring around a knob in the Mixer means that a lane sets it during playback.",
    ]),
    Heading("Return lanes"),
    Facts(&[
        "A return lane runs from silence at the bottom, through 100% at the dashed line, to 200% (about +6 dB) at the top.",
        "While it has points, it replaces the Return knob of its bus. To throw a delay or a reverb, keep the return low and raise it on the last hit of a phrase.",
    ]),
];

const RECORDING: &[Block] = &[
    Steps(&[
        "Choose the recording track in the selector beside Record. A free track becomes a new track.",
        "Press Record. Recording starts after a two-bar count-in.",
        "Press Stop.",
        "In the Recorded take window, Save take writes the take to a WAV file.",
    ]),
    Facts(&[
        "The take stays in memory until you save it. Before a new or opened song replaces it, the app asks whether to save it.",
        "Discarding a take asks once more, because it cannot be undone.",
    ]),
];

const MIX_VIEW: &[Block] = &[
    Facts(&[
        "Mix, in the score toolbar, turns each lane header into a mixer row beside its clips. The lower panel folds so the rows take the score. Arrange, or the rail's Mixer, returns to the lanes.",
        "The lanes fit the whole song, so the clips show where each track plays without scrolling.",
    ]),
    Heading("A row"),
    Facts(&[
        "From the left: the track number, the sounds, M, S, L, R, polarity, the level and its dB value, pan, the state, four sends, and EQ.",
        "A row at its defaults stays quiet. The state names only what differs: Muted, No output, Pan automated, or a pan off center.",
        "A track that cannot be heard is dimmed. While another track is soloed, the dimming and that track's lit S say why, and the hint on the sounds names Solo elsewhere.",
        "A send at 0 rests as a thin ring; its amount shows while it sends.",
        "EQ opens the track's channel window beside its row; the Mixer page explains the window.",
    ]),
    Keys(&[
        ("Reset", &[], "Double-click a fader or a send knob"),
        (
            "More columns",
            &["Shift+Wheel"],
            "when the rows are wider than the column",
        ),
    ]),
    Heading("Master and returns"),
    Facts(&[
        "Under the last row, Master holds Bypass, the master level, and Export tracks….",
        "The returns row holds the four FX buses: the effect, the return level, Input, Limiter, and Edit, which shows the bus on the FX returns board.",
        "A bus with the Empty effect returns silence, so its controls are dimmed.",
    ]),
];

const MIXER: &[Block] = &[
    Facts(&[
        "Mixer, in the tool rail, shows one strip per track. Mix, in the score toolbar, shows the same controls as rows beside the clips.",
        "Master stays in view while the tracks scroll.",
    ]),
    Heading("Level and pan"),
    Facts(&[
        "Gain runs from -inf dB (silence) to +12 dB; 0 dB is unity. Negative dB lowers the level; it does not invert polarity.",
        "Click a gain value to type an exact dB, then press Enter. Type -inf for silence.",
        "Unity, on the gain rail, sets 0 dB. Center, on the pan rail, resets the pan.",
        "The tapered fader gives 72% of its travel to -24 through +12 dB.",
    ]),
    Keys(&[
        ("Gain step", &["Arrows"], "0.5 dB"),
        ("Fine", &["Shift"], "0.1 dB, or fine dragging"),
        ("Silence", &["Home"], ""),
        ("+12 dB", &["End"], ""),
    ]),
    Heading("Outputs, Mute, and Solo"),
    Facts(&[
        "L and R switch each output. With both off, the track has no output.",
        "M mutes a track. S solos it: while any track is soloed, only soloed tracks play, and a muted track stays silent.",
        "The strip says why a silent track is silent: Muted, No output, or Solo elsewhere.",
        "The slashed-circle button reverses both channels' polarity without changing gain or pan. Amber means inverted; it stays set at silence.",
    ]),
    Heading("Channel window"),
    Facts(&[
        "EQ opens the track's channel window. The track passes its EQ, drive, compressor, and width, then its automation, fader, and pan, then its ducker and sends.",
        "The EQ has a low cut and a high cut at 12 dB per octave, a low shelf, two peaks with their Q, and a high shelf. Reset flattens every band. To tame harsh hi-hats, lower the high shelf.",
        "Drive adds harmonics, so a bass reaches small speakers. Below Focus the track stays clean; set it to about twice the bass's lowest note.",
        "The compressor evens out the level, for example the hits of a drum loop.",
        "Width: 0% is mono, 100% leaves the track as it is, and 200% doubles the stereo difference.",
        "The ducker lowers the track while its key track plays, for example a bass under the kick.",
        "A part that is off keeps its settings and leaves the track as it was. The EQ button lights when EQ, drive, compression, or ducking is on, or width differs from 100%.",
    ]),
    Terms(&[("Q", "The width of a peak band. A higher Q is narrower.")]),
    Heading("Sends and export"),
    Facts(&[
        "The four small knobs under the fader send the track, after its fader and pan, to FX 1 to FX 4.",
        "Bypass, on the Master strip, skips the master pedalboard for playback and export, as Mastering's Bypass all does.",
        "Export tracks…, in the Mixer's toolbar, writes each track that plays, and each FX return that it feeds, to its own WAV file in a folder.",
        "A track's file holds the track after its EQ, automation, fader, pan, and outputs, without its sends; the FX returns hold the sends. The master pedalboard is left out.",
        "Every file is as long as the mix export. The files sum to the mix with the board bypassed when that sum does not clip.",
    ]),
];

const MASTERING: &[Block] = &[
    Facts(&[
        "Mastering processes the whole mix after the tracks are summed. Open it with its button in the tool rail.",
        "The mix enters the Input unit, passes each pedal that is on from left to right, and leaves through the Output unit.",
        "Playback and export both use the board, so what you hear is what you export.",
    ]),
    Terms(&[
        (
            "dBFS",
            "Decibels below full scale. 0 dBFS is the loudest a file can hold.",
        ),
        (
            "dBTP",
            "True peak: the highest point of the wave between its samples.",
        ),
        (
            "LUFS",
            "Loudness as the ear hears it. -14 LUFS is a common streaming level.",
        ),
        ("VU", "A slow level meter. Here, 0 VU is -18 dBFS."),
    ]),
    Heading("Presets"),
    Facts(&[
        "Preset loads a factory board. Subtle, Revive, and Loud suit any finished mix, from a light polish to a loud master; the genre presets follow them.",
        "A preset sets every pedal and the order, turns the Booster pedals off, and ends at the Limiter with its ceiling at -1 dBTP.",
        "A preset keeps your Trim and the pedals' keys. One Undo restores the board before it.",
        "The selector reads Custom once you change a pedal. Songs save the board, not the preset's name.",
    ]),
    Heading("Input and Output"),
    Facts(&[
        "Trim sets the level into the board. The VU meter reads the trimmed mix; the mastering pedals work best near 0 VU. The peak lamps light at -1 dBFS.",
        "Momentary and short-term loudness follow the last 400 ms and 3 s. Integrated loudness and true peak cover the playback so far. The meters measure during playback.",
        "The green band marks -14 LUFS and the red line -1 dBTP. OVER lights when the true peak passes -1 dBTP, and stays lit until you click it.",
    ]),
    Heading("Pedals"),
    Facts(&[
        "Each pedal has an on/off switch at its foot and a lamp beside its name. A pedal that is off keeps its settings and passes the signal on.",
        "With every pedal off and Trim at 0 dB, the mix plays exactly as the tracks sum.",
        "The Booster pedals reproduce the master effects of three original releases: a ten-band EQ, a compressor, and stereo wide. Their EQ bands show decibels; Drive, Speed, Gain, and Width use the original 0 to 1000 scale.",
    ]),
    Terms(&[
        ("Soft clip", "Rounds peaks off."),
        ("Tube", "Adds warm, mostly even harmonics."),
        ("Trim EQ", "Cleans up rumble, mud, and harshness."),
        (
            "Bus compressor",
            "Glues the mix. Its lamps show the gain reduction.",
        ),
        ("Maximizer", "Adds loudness under a fixed ceiling."),
        ("Lift EQ", "Adds weight and air."),
        ("Tape", "Adds saturation with a low-end bump."),
        ("Stereo imaging", "Sets the width and keeps the lows mono."),
        (
            "Multiband",
            "Holds the peaks of the lows, mids, and highs apart and lifts the sound under them.",
        ),
        (
            "Limiter",
            "Keeps the true peak under its ceiling. Its lamps show the gain reduction.",
        ),
    ]),
    Facts(&[
        "Key, on the Booster compressor, Bus compressor, Multiband, and Limiter, sets what the pedal listens to. Own is the mix; a track number is that track after its fader. A muted key is silent.",
        "Some pedals delay the sound by a few milliseconds. The board's state beside Preset shows the total, and exports remove it.",
    ]),
    Heading("Order"),
    Facts(&[
        "Drag a pedal by its name to move it; near either end the row scrolls. The arrow buttons move a pedal one place. Each move is one Undo step.",
        "The Limiter cannot move. It always runs last, just before the Output unit, so no pedal can raise the peaks it holds.",
        "The strip below the board shows every pedal in order; click one to bring it into view. In a short window, scroll the board to reach the strip.",
    ]),
    Heading("Bypass and songs"),
    Facts(&[
        "Bypass all, beside Preset, skips the whole board, Trim included, for playback and export, so you can master the mix elsewhere. The pedals keep their settings, and the board dims.",
        "Switching Bypass all during playback crossfades over 30 ms. Presets leave it as it is. The Mixer's Master strip has the same switch.",
        "Songs open with every pedal off unless their file switched one on. A legacy .mix song applies its saved Booster settings; a MixJam v7 project applies its master bus.",
    ]),
    Heading("Knobs"),
    KNOBS,
    Facts(&["Changes apply while the song plays, and Undo restores them."]),
];

const FX_RETURNS: &[Block] = &[
    Facts(&[
        "FX returns, in the tool rail, shows the four FX buses that the Mixer's sends feed. The returns join the mix before the master pedalboard.",
    ]),
    Steps(&[
        "Choose Echoform Delay, Aetherform Reverb, or Emberform Drive for the bus.",
        "Load a preset.",
        "Set its controls.",
    ]),
    Facts(&[
        "Sync sets the delay in note lengths at the song tempo.",
        "Input off lets the tail ring on. Clear tail stops it at once. An export lets the tails continue after the end of the song.",
        "Each effect's ducker lowers its repeats or tail while its input plays. Set Key to a track number, such as the kick's, to follow that track instead. A preset keeps the Key.",
        "The limiter of each bus keeps its return below -1 dBFS.",
        "In the Mixer, each return strip sets the return level and switches the Input and Limiter of its bus. Edit opens the bus here.",
    ]),
    Heading("Emberform Drive"),
    Facts(&[
        "A parallel distortion: the dry sends stay clean, and the bus adds saturation beside them. It has no delay, so it stays in phase with the dry sound.",
        "Type sets the curve. Drive pushes the input into it at a steady output level. Asym adds even harmonics.",
        "Low cut and High cut set the band that it distorts, so a bass can keep a clean sub.",
    ]),
];

const WORKSPACE: &[Block] = &[
    Heading("Inspector"),
    Facts(&[
        "The Inspector, at the right of the score, shows what is selected. With nothing selected, it shows the song: its tempo, its length, a clip count for each category, and its sections.",
        "Click a track's name in its lane header to see its channel processing, level, pan, outputs, and sends; click the EQ curve to open the channel EQ window, and the name again to see the song.",
        "One clip shows a picture of its length and fades, whose handles you can drag, and its fades, sound, and position as exact values. Several clips share the fade and sound values: a change sets them all, and a dash marks a value that differs.",
        "Click a section in the Inspector to scroll the score to it. Every change in the Inspector can be undone.",
    ]),
    Heading("Panels"),
    Facts(&[
        "Panels, in the status bar, shows or folds Tools, Boards, Sounds, Overview, and Inspector. In a small window the panels share the room, and the one you opened last shows.",
        "Ctrl+K lists every command with its shortcut.",
    ]),
    Heading("Files"),
    Facts(&[
        "Open reads MIX, MIXJAM, and JSON songs. Save writes MIXJAM. Export writes a stereo WAV of the mix.",
        "F12 saves the last 30 seconds that playback sent to the audio device, with the song's state, to the diagnostics folder in the data folder. Press it while something sounds wrong.",
    ]),
];

const STRETCH: &[Block] = &[
    Facts(&[
        "Original BPM describes the recording. Target BPM sets the result to 120, 125, 130, 135, 140, 160, or 180 BPM.",
        "A lower target makes the sound longer; a higher target makes it shorter.",
        "Pitch preservation is approximate and can change transients or add audible artifacts. Listen before saving.",
    ]),
    Heading("Controls"),
    Keys(&[
        (
            "Original BPM",
            &[],
            "Click it to type. Drag the knob, turn the wheel, or press an arrow; Shift drags finely.",
        ),
        (
            "Target BPM",
            &[],
            "Click or drag to a notch, turn the wheel, or press an arrow.",
        ),
        (
            "Reset",
            &[],
            "Double-click Original for the source tempo, or Target for 140 BPM.",
        ),
    ]),
    Facts(&[
        "Listen changes to Stop listening during preview. The status line reports processing, playback, errors, and a saved sound.",
        "Saving keeps the source and both tempos, so another version is one adjustment away.",
        "Esc closes the tool. It first closes an open list or cancels a value being typed.",
    ]),
];

const EFFECTS: &[Block] = &[
    Facts(&[
        "Select a palette sound or a clip, then open Effect Studio. Listen previews the processed sound. Save sound writes a new WAV, adds it to Effect, and selects it.",
        "Filter, Echo, and Robotize each have an on/off switch beside their name. Turning a section off discards its settings; turning it on restores its defaults.",
        "Dirty distortion, Noise distortion, and Output volume are always available. Reset restores all defaults.",
        "Cutoff and Color use logarithmic ranges. Echo delay is measured in beats at the source tempo. Echo tails stop at the sample boundary.",
    ]),
    Heading("Knobs"),
    KNOBS,
    Facts(&[
        "The upper-left triangle collapses the window without changing its settings.",
        "Esc closes the tool after a focused value, list, or window above it has handled the key.",
        "The effects are the application's own algorithms.",
    ]),
];

const STEP_SYNTH: &[Block] = &[
    Heading("Source"),
    Facts(&[
        "Source file… chooses a WAV file, or a PXD file: the compressed sound format of the original products. Palette sound uses the selected palette sound or clip.",
        "Start is where each note starts to read the source, in ms. Find start sets it to the source's first hit, so the notes play on the beat.",
        "A new source sets Start to its first hit and turns Loop source off; a loaded pattern keeps its own. Loop source repeats audio while a note sounds.",
    ]),
    Heading("Listen and save"),
    Keys(&[("Listen / Stop", &["L"], "while the board shows")]),
    Facts(&[
        "Save sound writes a WAV to Step synth at the pattern's tempo.",
        "A pattern that would clip plays and saves turned down as a whole, and the status line says by how much.",
        "The pattern plays as a loop, so the echo and filter tails at its end come back at its start.",
    ]),
    Heading("Grid"),
    Facts(&[
        "The pattern has four bars of sixteen steps. The grid shows four, two, or one bar, as many as the board's width allows; the bar buttons bring a bar into view.",
        "25 pitch rows show +12 to -12 semitones around the octave in view, and the olive 0 row keeps the source pitch. The grid opens on the octave nearest the notes.",
        "The − and + keys beside the step ruler, or Page Up and Page Down, shift the rows by up to three octaves. A coral edge marks a note above or below them.",
        "Scale and its root, A to G# with A as the source pitch, shade the rows outside the scale but never block a pitch. Randomise writes a new line in the scale over the pattern's length.",
    ]),
    Heading("Notes"),
    Keys(&[
        (
            "Place / clear",
            &[],
            "Click a cell; click a note to clear it",
        ),
        ("Hold longer", &[], "Drag a note's last step sideways"),
        ("Accent", &["Shift"], "click a note, or use the Accent lane"),
        ("Slide", &["Alt"], "click a note, or use the Slide lane"),
        ("Velocity", &[], "Drag on the Velocity lane"),
        (
            "Select steps",
            &[],
            "Drag along the step ruler; lane edits then apply to all of them",
        ),
    ]),
    Facts(&[
        "Accent sets how much louder an accented note plays, up to 12 dB. Slide time sets how long a slide takes to reach the next pitch, from 1 to 1000 ms.",
        "Swing delays every second sixteenth, from 50% (straight) to 75%. With swing, a note on the beat sounds longer than one off the beat.",
        "Gate sets how much of its last step a note sounds for; a note that slides into the next ignores it. Every note ends with a short fade, so a cut does not click.",
    ]),
    Heading("Pattern"),
    Facts(&[
        "Length, under Pattern, sets how many steps repeat; drag its end marker on the ruler. The steps after it keep their notes.",
        "×2 and ÷2 double and halve the length; ×2 repeats the loop into the new steps when they are empty.",
        "Copy to copies the viewed bar, the one picked last, onto the next bar. Fill copies it onto every bar, and Clear bar empties it. Clear, under Pattern, empties every bar.",
        "Notes and Effects, at the left of the top row, switch between the note grid and the pattern effects. While Listen plays, the strip beside them lights the playing step.",
        "Load and Save, under Pattern, keep the editable draft in JSON with its source file. Load switches to that source, or to the palette sound when the file is missing, and keeps the viewed bar.",
        "The tempo choices are 120, 125, 130, 135, 140, 160, and 180 BPM. A loaded tempo between them moves to the nearest one, and the status line says so.",
    ]),
    Heading("Undo"),
    Facts(&[
        "With the pointer over the board, or the focus in it, Ctrl+Z takes back the board's last edit of a note, setting, effect, source, or load. Ctrl+Y or Ctrl+Shift+Z puts it back. Elsewhere they act on the song.",
        "The undo and redo keys in the top row do the same, and the status line names the edit.",
    ]),
    Heading("Keyboard"),
    Keys(&[
        (
            "Next control",
            &["Tab"],
            "reaches each control and the note grid",
        ),
        (
            "Grid cell",
            &["Arrows"],
            "the focused cell scrolls into view",
        ),
        ("First / last", &["Home", "End"], "steps in view that play"),
        ("Bar page", &["Ctrl+Left", "Ctrl+Right"], ""),
        ("Octave", &["Page Up", "Page Down"], ""),
        ("Place / clear", &["Space", "Enter"], "in the grid"),
        ("Step cursor", &["Left", "Right"], "on the ruler and lanes"),
        (
            "Select range",
            &["Shift+Left", "Shift+Right"],
            "on the ruler",
        ),
        (
            "Accent / Slide",
            &["Space", "Enter"],
            "toggles; Up or Down sets or clears",
        ),
        ("Velocity", &["Up", "Down"], "one level"),
    ]),
    Facts(&[
        "Lane edits also apply to the selected notes. Esc first cancels an active value or list, or goes to a window above the board.",
        "The Step synth button in the tool rail folds the lower panel without changing the draft; choose it again to open the board.",
        "Resampling, source looping, and effects are the application's own algorithms.",
    ]),
];

const SAMPLE_EDITOR: &[Block] = &[
    Facts(&[
        "Select a palette sound or a clip, then open Sample editor. Listen edit previews the result. Save edited sound writes a new WAV, adds it to Wave, and leaves the source unchanged.",
        "Start frame is the first frame that is kept, counted from zero. End frame is the first frame that is left out; set it to the frame count to keep the last frame. A frame is one sample of audio.",
        "Pitch runs from -24 to +24 semitones and changes speed and length together: +12 is an octave up and half as long, -12 an octave down and twice as long.",
    ]),
    Heading("Controls"),
    Keys(&[
        (
            "Exact value",
            &["Enter"],
            "after you click a value and type",
        ),
        (
            "Frame step",
            &["Shift"],
            "+ and − change one frame; with Shift, 100",
        ),
        (
            "Pitch step",
            &["Shift"],
            "the buttons change 0.1 semitone; with Shift, one",
        ),
    ]),
    Facts(&[
        "The status line shows the selected frames and the length after pitch, or why editing is unavailable. A missing sound plays silence; resolve it in Import details.",
        "The upper-left triangle collapses the tool and keeps the edit. Esc closes the tool after a value being typed or a window above it has handled the key.",
    ]),
];

const ALBUM_REVIEW: &[Block] = &[
    Facts(&[
        "Album Review is a library of deep-listening analyses of albums, and of your own albums and songs in the same form. Open it with Review in the tool rail.",
    ]),
    Heading("Find an album"),
    Facts(&[
        "Search finds titles, artists, and genre tags, and ignores accents.",
        "Artist filters show the albums of each selected artist. Genre filters show the albums that have every selected tag; a tag that would leave no album is disabled, and +N more shows the rest.",
        "Active filters show as chips. Clear filters removes them and the search. Sort by year, artist, title, or track count.",
    ]),
    Heading("Album page"),
    Facts(&[
        "Select a card to open its page: the metadata, the overview, the listening links, a timeline with one row per track, and the events of each track.",
        "Select a timeline row to go to its track.",
    ]),
    Heading("Covers and files"),
    Facts(&[
        "Display cover art downloads the covers into the cover cache of the data folder, up to 256 MB. Clear cover cache removes them.",
        "Reload reviews reads the reviews folder again after its files change.",
    ]),
];

/// The width of the page list of the main window.
const LIST_WIDTH: f32 = 156.0;
/// Below this width the page list wraps above the page.
const NARROW: f32 = 560.0;
/// The widest line of a page: about 75 characters of body text.
const MEASURE: f32 = 540.0;
/// The action column of key and term rows.
const ACTION_WIDTH: f32 = 128.0;

/// Remember the lower panel, so that a board's Help opens beside it.
pub(super) fn note_board_rect(ctx: &egui::Context, rect: Rect) {
    ctx.data_mut(|data| data.insert_temp(egui::Id::new("help-board-rect"), rect));
}

/// What a page's window opens beside: its tool window, or the lower panel.
pub(super) fn anchor(ctx: &egui::Context, topic: Topic) -> Option<Rect> {
    match topic.tool_window() {
        Some(tool) => dialog::window_rect(ctx, tool),
        None => ctx.data(|data| data.get_temp::<Rect>(egui::Id::new("help-board-rect"))),
    }
}

/// The main Help window's body: the page list and the current page.
pub(super) fn manual(ui: &mut egui::Ui, current: &mut Topic) {
    let identity = "Help";
    let mut list_focus = false;
    if ui.available_width() < NARROW {
        // Keep a page viewport even at the shell's minimum size.
        let list_height = (ui.available_height() * 0.4).max(26.0);
        egui::ScrollArea::vertical()
            .id_salt("help-topics")
            .min_scrolled_height(0.0)
            .max_height(list_height)
            .auto_shrink([false, true])
            .show(ui, |ui| {
                ui.horizontal_wrapped(|ui| {
                    ui.spacing_mut().item_spacing = Vec2::new(4.0, 4.0);
                    for topic in Topic::ALL {
                        list_focus |= list_item(ui, topic, current, true);
                    }
                });
            });
        view::name_scroll_area(ui, "help-topics", "Help pages");
        ui.add_space(4.0);
        ui.separator();
        page_scroll(ui, identity, *current, true, list_focus);
        return;
    }
    let height = ui.available_height();
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(LIST_WIDTH, height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(LIST_WIDTH);
                egui::ScrollArea::vertical()
                    .id_salt("help-topics")
                    .auto_shrink([false, false])
                    .max_height(height)
                    .show(ui, |ui| {
                        ui.spacing_mut().item_spacing.y = 2.0;
                        for topic in Topic::ALL {
                            if topic == Topic::Stretch {
                                ui.add_space(6.0);
                                ui.separator();
                                ui.add_space(6.0);
                            }
                            list_focus |= list_item(ui, topic, current, false);
                        }
                    });
                view::name_scroll_area(ui, "help-topics", "Help pages");
            },
        );
        let x = ui.cursor().left() + 2.0;
        ui.painter().line_segment(
            [
                Pos2::new(x, ui.min_rect().top()),
                Pos2::new(x, ui.min_rect().top() + height),
            ],
            Stroke::new(1.0_f32, Color32::from_black_alpha(110)),
        );
        ui.add_space(14.0);
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), height),
            egui::Layout::top_down(egui::Align::Min),
            |ui| page_scroll(ui, identity, *current, true, list_focus),
        );
    });
}

/// A tool's Help window body: its page, without the page list.
pub(super) fn tool_page(ui: &mut egui::Ui, topic: Topic) {
    page_scroll(ui, topic.window().unwrap_or("Help"), topic, false, false);
}

/// One entry of the page list. Arrows move between entries; Tab leaves.
/// Returns whether the entry has the keyboard focus.
fn list_item(ui: &mut egui::Ui, topic: Topic, current: &mut Topic, compact: bool) -> bool {
    let name = topic.name();
    let galley =
        ui.painter()
            .layout_no_wrap(name.to_owned(), FontId::proportional(13.0), roles().legend);
    let mark = if topic.palette().is_some() { 14.0 } else { 0.0 };
    let width = if compact {
        galley.size().x + mark + 20.0
    } else {
        ui.available_width()
    };
    let (rect, _) = ui.allocate_exact_size(Vec2::new(width, 26.0), Sense::hover());
    let response = ui.interact(
        rect,
        egui::Id::new(("help-page-entry", name)),
        Sense::click(),
    );
    let focus_id = egui::Id::new("help-list-focus");
    if ui.data(|data| data.get_temp::<Topic>(focus_id)) == Some(topic) {
        response.request_focus();
        response.scroll_to_me(None);
        ui.data_mut(|data| data.remove::<Topic>(focus_id));
    }
    // A click gives the entry the focus, so the arrows go on from it, but
    // only the keyboard shows the focus ring.
    let pointer_id = egui::Id::new("help-list-pointer");
    if response.clicked() {
        *current = topic;
        response.request_focus();
        ui.data_mut(|data| data.insert_temp(pointer_id, true));
    }
    if response.has_focus() {
        // The list keeps Up and Down; egui would move the focus with them.
        ui.memory_mut(|memory| {
            memory.set_focus_lock_filter(
                response.id,
                egui::EventFilter {
                    vertical_arrows: true,
                    ..Default::default()
                },
            )
        });
        let index = Topic::ALL.iter().position(|t| *t == topic).unwrap_or(0);
        let step = ui.input_mut(|input| {
            if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowDown) {
                Some(index + 1)
            } else if input.consume_key(egui::Modifiers::NONE, egui::Key::ArrowUp) {
                index.checked_sub(1)
            } else {
                None
            }
        });
        if let Some(next) = step.and_then(|index| Topic::ALL.get(index).copied()) {
            *current = next;
            ui.data_mut(|data| {
                data.insert_temp(focus_id, next);
                data.insert_temp(pointer_id, false);
            });
        }
    }
    if response.gained_focus() && ui.input(|input| input.key_pressed(egui::Key::Tab)) {
        response.scroll_to_me(None);
        ui.data_mut(|data| data.insert_temp(pointer_id, false));
    }
    let ring =
        response.has_focus() && !ui.data(|data| data.get_temp::<bool>(pointer_id).unwrap_or(false));
    let selected = *current == topic;
    response.widget_info(|| {
        egui::WidgetInfo::selected(
            egui::WidgetType::SelectableLabel,
            true,
            selected,
            format!("Help page: {name}"),
        )
    });
    let corner = roles().corner(3.0);
    let painter = ui.painter();
    if selected {
        painter.rect(
            rect,
            corner,
            roles().face,
            Stroke::new(1.0_f32, roles().widget_edge),
            egui::StrokeKind::Inside,
        );
    } else if response.hovered() {
        painter.rect_filled(rect, corner, Color32::from_white_alpha(8));
    }
    if ring {
        painter.rect_stroke(
            rect.shrink(1.0),
            corner,
            Stroke::new(1.5_f32, ui.visuals().selection.bg_fill),
            egui::StrokeKind::Inside,
        );
    }
    let mut x = rect.left() + 10.0;
    if let Some(palette) = topic.palette() {
        painter.rect_filled(
            Rect::from_center_size(Pos2::new(x + 3.5, rect.center().y), Vec2::splat(7.0)),
            1.5,
            palette.accent(),
        );
        x += mark;
    }
    let y = rect.center().y - galley.size().y / 2.0;
    painter.galley(Pos2::new(x, y), galley, roles().legend);
    response.has_focus()
}

/// A page in its own scroll area. Page Up, Page Down, Home, and End scroll
/// it while the pointer is over it or the page list has the focus.
fn page_scroll(ui: &mut egui::Ui, identity: &str, topic: Topic, title: bool, list_focus: bool) {
    let salt = ("help-page", identity);
    let shown = egui::Id::new(salt).with("topic");
    let metrics = egui::Id::new(salt).with("metrics");
    let mut area = egui::ScrollArea::vertical()
        .id_salt(salt)
        .auto_shrink([false, false])
        .min_scrolled_height(0.0)
        .max_height(ui.available_height());
    if ui.data(|data| data.get_temp::<Topic>(shown)) != Some(topic) {
        ui.data_mut(|data| data.insert_temp(shown, topic));
        area = area.vertical_scroll_offset(0.0);
    } else if let Some(last) = ui.data(|data| data.get_temp::<PageMetrics>(metrics))
        && let Some(offset) = keyboard_offset(ui, identity, last, list_focus)
    {
        area = area.vertical_scroll_offset(offset);
    }
    let output = area.show(ui, |ui| page(ui, topic, title));
    ui.data_mut(|data| {
        data.insert_temp(
            metrics,
            PageMetrics {
                view: output.inner_rect,
                offset: output.state.offset.y,
                end: (output.content_size.y - output.inner_rect.height()).max(0.0),
            },
        )
    });
    view::name_scroll_area(ui, salt, &format!("{} page", topic.name()));
}

/// Where a page showed last frame and how far it scrolled.
#[derive(Clone, Copy)]
struct PageMetrics {
    view: Rect,
    offset: f32,
    end: f32,
}

/// The offset that Page Up, Page Down, Home, or End asks for. The keys
/// jump at once, as a reader's do.
fn keyboard_offset(
    ui: &egui::Ui,
    identity: &str,
    last: PageMetrics,
    list_focus: bool,
) -> Option<f32> {
    let ctx = ui.ctx();
    let focused = ctx.memory(|memory| memory.focused());
    let own_focus = focused
        .and_then(|id| ctx.read_response(id))
        .is_some_and(|response| response.layer_id == ui.layer_id());
    let hovered = ctx
        .pointer_hover_pos()
        .is_some_and(|at| last.view.contains(at) && ctx.layer_id_at(at) == Some(ui.layer_id()));
    if !(list_focus || own_focus || focused.is_none() && hovered)
        || !dialog::top_window(ctx, identity)
        || egui::Popup::is_any_open(ctx)
    {
        return None;
    }
    let page = (last.view.height() - 40.0).max(40.0);
    ctx.input_mut(|input| {
        [
            (egui::Key::PageDown, last.offset + page),
            (egui::Key::PageUp, last.offset - page),
            (egui::Key::End, last.end),
            (egui::Key::Home, 0.0),
        ]
        .into_iter()
        .find(|(key, _)| input.consume_key(egui::Modifiers::NONE, *key))
        .map(|(_, offset)| offset.clamp(0.0, last.end))
    })
}

fn page(ui: &mut egui::Ui, topic: Topic, title: bool) {
    let width = ui.available_width().min(MEASURE);
    ui.set_max_width(width);
    ui.spacing_mut().item_spacing = Vec2::new(8.0, 7.0);
    if title {
        heading(ui, topic.name(), 16.0, 1);
        ui.add_space(2.0);
    }
    for (index, block) in topic.blocks().iter().enumerate() {
        match block {
            Heading(text) => {
                if index > 0 || title {
                    ui.add_space(10.0);
                }
                heading(ui, text, 13.0, 2);
            }
            Facts(lines) => {
                for line in *lines {
                    ui.add(egui::Label::new(*line).wrap());
                }
            }
            Steps(steps) => {
                for (number, step) in steps.iter().enumerate() {
                    columns(
                        ui,
                        22.0,
                        |ui| {
                            ui.label(action_text(&format!("{}.", number + 1)));
                        },
                        |ui| {
                            ui.add(egui::Label::new(*step).wrap());
                        },
                    );
                }
            }
            Keys(rows) => {
                for (action, keys, note) in *rows {
                    columns(
                        ui,
                        ACTION_WIDTH,
                        |ui| {
                            // The same wrapped row as the keys, so that both
                            // columns centre on one line.
                            ui.horizontal_wrapped(|ui| {
                                ui.add(egui::Label::new(action_text(action)).wrap());
                            });
                        },
                        |ui| {
                            ui.horizontal_wrapped(|ui| {
                                ui.spacing_mut().item_spacing.x = 5.0;
                                for (index, combo) in keys.iter().enumerate() {
                                    if index > 0 {
                                        ui.label(egui::RichText::new("/").color(roles().help));
                                    }
                                    keycap(ui, combo);
                                }
                                if !note.is_empty() {
                                    ui.add(egui::Label::new(*note).wrap());
                                }
                            });
                        },
                    );
                }
            }
            Terms(rows) => {
                for (term, meaning) in *rows {
                    columns(
                        ui,
                        ACTION_WIDTH,
                        |ui| {
                            ui.add(egui::Label::new(action_text(term)).wrap());
                        },
                        |ui| {
                            ui.add(egui::Label::new(*meaning).wrap());
                        },
                    );
                }
            }
        }
    }
    ui.add_space(6.0);
}

/// A heading that assistive technology announces as one, at `level`.
fn heading(ui: &mut egui::Ui, text: &str, size: f32, level: usize) {
    let response = ui.add(egui::Label::new(view::heading(text, size).color(roles().legend)).wrap());
    ui.ctx().accesskit_node_builder(response.id, |node| {
        node.set_role(Role::Heading);
        node.set_level(level);
    });
}

/// The action or term of a row, in the heading face, so the eye finds it.
fn action_text(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .font(FontId::new(13.0, theme::heading_family()))
        .color(roles().legend)
}

fn columns(
    ui: &mut egui::Ui,
    left: f32,
    first: impl FnOnce(&mut egui::Ui),
    second: impl FnOnce(&mut egui::Ui),
) {
    let left = left.min(ui.available_width() * 0.4);
    ui.horizontal_top(|ui| {
        ui.allocate_ui_with_layout(
            Vec2::new(left, 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(left);
                first(ui);
            },
        );
        ui.allocate_ui_with_layout(
            Vec2::new(ui.available_width(), 0.0),
            egui::Layout::top_down(egui::Align::Min),
            |ui| {
                ui.set_width(ui.available_width());
                second(ui);
            },
        );
    });
}

/// A key or key combination, drawn as a key cap.
fn keycap(ui: &mut egui::Ui, keys: &str) {
    let galley =
        ui.painter()
            .layout_no_wrap(keys.to_owned(), FontId::proportional(12.0), roles().legend);
    let size = Vec2::new(galley.size().x + 12.0, galley.size().y.max(14.0) + 5.0);
    let (rect, response) = ui.allocate_exact_size(size, Sense::hover());
    response.widget_info(|| egui::WidgetInfo::labeled(egui::WidgetType::Label, true, keys));
    let corner = roles().corner(3.0);
    let painter = ui.painter();
    painter.rect(
        rect,
        corner,
        Color32::from_white_alpha(8),
        Stroke::new(1.0_f32, roles().widget_edge),
        egui::StrokeKind::Inside,
    );
    // The cap's lower edge, as on a keyboard.
    painter.line_segment(
        [
            Pos2::new(rect.left() + corner + 1.0, rect.bottom() - 1.0),
            Pos2::new(rect.right() - corner - 1.0, rect.bottom() - 1.0),
        ],
        Stroke::new(1.0_f32, Color32::from_black_alpha(110)),
    );
    painter.galley(rect.center() - galley.size() / 2.0, galley, roles().legend);
}
