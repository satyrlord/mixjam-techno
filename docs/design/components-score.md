# Components: Inspector, overview, and top bar

## Contents

[Score toolbar and
Maximise](components-score-toolbar.md#score-toolbar-and-maximise) has its own
page.

## Inspector

The Inspector is a 280-point column at the right of the score, above the
lower panel, on Panel fill with 12-point and 10-point margins. A muted
Inspector label and a fold chevron are at its top. Below them, one page
scrolls. The selection chooses the page:

- Nothing selected: the song. The page shows the title in the heading face,
  the tempo and length, and two lists:
  - CATEGORIES: a swatch, the name, and the clip count of each category, in
    the order of the Sounds panel, with missing sounds last.
  - SECTIONS: a 3-point bar in the color of each section, its name, and its
    bars. A click on a section row scrolls the score to that section. It
    never moves the playhead.
- A track: a click on its name in the Arrange lane header selects it. The
  click also opens a folded Inspector, and the name then has Raised fill and a
  2-point accent edge. If the Inspector is open on the track, a second click
  on the name shows the song. The page shows:
  - CHANNEL PROCESSING: the EQ response curve and the EQ, Drive, Compressor,
    and Ducker switches. The curve is the button that opens the channel EQ
    window with every dial: "Open EQ" in its corner, and an accent edge on
    hover.
  - CHANNEL: one strip card of three rows. The first row has the strip
    fader, the dB field, and Unity. The second row has the pan slider,
    Center, and the pan, or "Automated" while a lane drives it. The third row
    has Mute, Solo, Left, Right, polarity, and, only for a silent track, why
    it is silent. A single row cannot fit the 256-point column, so the rows
    stack in one card.
  - SENDS: the four send knobs, each with its return's effect and the send
    amount below it. The knob of an empty or switched-off return is dimmed.
- One clip: the sound's name, then two lines: its category, and its bars and
  track. Below them is a picture of the clip at the panel's width. The picture
  shows its category fill and waveform, its shaded fades, its gain line, and a
  tick at each bar. It also has the two fade handles, which drag as on the
  clip in the score. The arrow keys also adjust a focused handle. Then come
  three strip cards of fields, each label at the left and each value in one
  column:
  - FADES (BEATS): each fade length and its curve as a drop-down list.
  - SOUND: the gain as a strip fader and a dB field, the pitch in semitones
    and cents, and Reverse as a switch.
  - POSITION: the start bar, the length in bars, and the trim start in beats
    at the song's tempo.
  If the sound of a clip is missing, the page shows a note and "Locate the
  sound…". This button asks for the file, as the clip menu's Resolve manually
  does.
- Several clips: their count, span, and tracks, the FADES and SOUND cards,
  and their category counts. A change in a card sets every selected clip, as
  one undo step. A dash marks mixed numbers and fade curves. Reverse shows
  "Some play backwards" for a mixed selection. The two pitch fields adjust
  semitones and cents separately, within the clip's pitch limits.

The controls have the names "Inspector track ..." and "Inspector clip ...".
These names never match a name in the Mixer, the Mix view, or the clip menu.
Every edit is one undo step, and the handles on the clips stay. The scroll bar
takes its own room, so it never covers a card. The folded strip is 28 points
wide, with a chevron and INSPECTOR read top to bottom. A click anywhere on the
strip opens the Inspector. A maximised score hides both.

## Room for the panels

The Sounds panel and the Inspector share the width of the score. The Sounds
panel and the board of the lower panel share its height. Both side panels
open together if 520 points stay between them. This is from 1134 points of
window width (1114 with the icon rail). The Sounds panel stays open above a
board when it keeps 330 points: its search, filters, chips, sort, and three
rows.

If a pair does not fit, the panel that opened most recently shows. The other
panel folds to its strip, or the lower panel folds away. The saved
layout keeps each choice, so a larger window shows all the panels again. If a
window is too narrow for both side panels, it starts with the Inspector
folded. The Inspector stays folded until the user opens it. At 960x720 and
1024x768, the Sounds list then gets the whole height, not one or two rows
beside the Mixer.

## Song overview

A 60-point strip above the score toolbar shows the whole song, on Panel fill
with a 4-point radius. It spans the end of the song, its last section, and the
playhead. It also spans at least the visible width of the score, so a short or
empty song keeps its true size. Inside a 12-point inset, from left to right,
it shows:

- With Sections on, a 2-point line in the section color along the top of each
  section, and its name in uppercase 10-point text. The current section uses
  Text and the other sections use Muted. Every second section has a shade of
  Canvas at 55%.
- Each clip is one thin line in its category color. Each shown track has one
  row. The lines use the same ticks as the clip rectangles of the score. A
  missing sound draws in Raised. A muted or solo-silenced track mixes 30% of
  its color into Canvas. The lines are one mesh. The application builds the
  mesh again only if the song, catalog, theme, strip, or span changes.
- The loop, as a 2-point accent line along the bottom.
- The visible window of the score, as a frame: Text at 8% fill and an 85%
  1-point stroke, inside the strip.
- The playhead, as a 1.5-point accent line.

A click or drag centers the score view on that point. With the strip focused,
Left and Right move it by one screen, and Home and End go to the start and the
end. The overview never edits the song or moves the playhead.

A 22-point hide button with an up chevron sits over the lower right corner.
When folded, a 20-point bar with a down chevron and SONG OVERVIEW opens the
overview again from anywhere on the bar. An 8-point gap separates the strip or
the bar from the toolbar.

If the score has no room for the open strip together with the toolbar, the
ruler, and a 150-point lane viewport, the bar shows. The bar then reads
"Needs a taller score", and a click on it does nothing. The saved choice stays.
If the bar also cannot fit in that score area, the overview hides until
there is space again. The Overview status switch stays available. This rule
keeps the 150-point lane viewport and the full Mastering board from 1024x768.
The application saves the choice in `layout.json`.

## Top bar

One 64-point top bar spans the window, with 18-point side margins and
8 points above and below its 48-point row. From the left, it holds:

1. the tribal mask,
2. the brand,
3. the song name,
4. the eight transport buttons,
5. the microphone and the recording-track selector,
6. the position readout,
7. the BPM label and tempo slider,
8. at the right end, Undo, Redo, and Help.

The transport buttons are, in order:

- Loop,
- Go to start,
- Back one bar,
- Stop,
- Play / pause (Space),
- Forward one bar,
- Go to end,
- Record into the armed track.

Tab follows the same order. Below 1240 points of content, the bar is compact.
A compact bar has tighter spacing, a smaller brand, no separator after the
brand, and a narrower readout, tempo, and action buttons. Every control then
stays on screen at the minimum window size. Help is only in the top bar.

The top bar puts a 40-point tribal mask before the MixJam Techno brand. The
brand is MixJam over a muted Techno, at 20 and 11 points, or at 14 and
9 points in a compact bar. The artwork keeps its original colors and
transparent background. The native window, the taskbar, and the Windows
executable use the same preserved mask. `assets/app/` contains its PNG and
its ICO with several sizes. The viewport embeds the PNG, and `build.rs` links
the ICO into the executable.

Transparent PNG artwork supplies alpha masks. `src/ui_icons.rs` keeps their
alpha and normalizes RGB to white. The view applies Text, Legend, Muted, or
Accent tints. The view caches textures per egui context. Tooltips and
accessibility labels give the action names. The recording destination stays
a visible track number. The canonical PNG artwork and its provenance stay in
`assets/rail/`. The project must not have SVG files or embedded SVG artwork
in tracked or non-ignored files. Ignored `local-data/` and `tmp/` files are
outside this check. Original preservation material stays in `local-data/`
under the layout of the owner.

Control icons use prepared 20-pixel PNGs, and the title mask uses a 40-pixel
PNG, at 100% Windows scaling, the only scale in scope. These are the only
prepared sizes. Each image texel maps to exactly one physical screen pixel,
with pixel-aligned bounds and no resize at runtime. The Windows ICO for the
shell icon is generated from the canonical PNG.
