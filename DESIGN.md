---
name: MixJam Techno
description: A modern native studio for arranging, mixing, and shaping classic sounds.
colors:
  canvas: "#14171b"
  panel: "#1c2025"
  text: "#e9edee"
  accent: "#b8e37e"
  coral: "#ff8282"
  muted: "#a4aeb6"
  line: "#3a4148"
  raised: "#272d33"
  hover: "#333b42"
  selection: "#415532"
  warning: "#e8c585"
  loop: "#d4c182"
  drum: "#d99d78"
  bass: "#a2c485"
  sequence: "#89b0d2"
  sphere-wave: "#7fbeb9"
  effect: "#d99192"
  voice: "#b99ecb"
  other-sample: "#bbb39f"
  stretch-cream: "#f5e8cd"
  stretch-ink: "#181917"
  stretch-body: "#2c2e2e"
  stretch-body-edge: "#0c0c0b"
  stretch-well: "#182526"
  stretch-well-line: "#445f60"
  stretch-well-deep: "#0e0f0f"
  stretch-well-hover: "#283c3d"
  stretch-well-selected: "#3a5455"
  stretch-error: "#ffa78f"
  stretch-lamp-rim: "#807b6e"
  effects-cream: "#f5e8cd"
  effects-ink: "#181917"
  effects-body: "#2c2e2e"
  effects-well: "#1e1715"
  effects-well-line: "#5a413a"
  effects-error: "#ffa78f"
typography:
  display:
    fontFamily: "Syncopate"
    textTransform: "uppercase"
    fontSize: "27px"
    fontWeight: 700
  headline:
    fontFamily: "Syncopate"
    textTransform: "uppercase"
    fontSize: "20px"
    fontWeight: 700
  section:
    fontFamily: "Syncopate"
    textTransform: "uppercase"
    fontSize: "18px"
    fontWeight: 700
  title:
    fontFamily: "Syncopate"
    textTransform: "uppercase"
    fontSize: "15px"
    fontWeight: 700
  body:
    fontFamily: "Inter"
    fontSize: "13px"
    fontWeight: 500
  button:
    fontFamily: "Inter"
    fontSize: "13px"
    fontWeight: 500
  clip:
    fontFamily: "Inter"
    fontSize: "12px"
    fontWeight: 500
  small:
    fontFamily: "Inter"
    fontSize: "11px"
    fontWeight: 500
  metadata:
    fontFamily: "Inter"
    fontSize: "10px"
    fontWeight: 500
  stretch-title:
    fontFamily: "Syncopate"
    textTransform: "uppercase"
    fontSize: "29.09px"
    fontWeight: 700
  effects-title:
    fontFamily: "Syncopate"
    fontSize: "22px"
    fontWeight: 700
  effects-value:
    fontFamily: "Inter"
    fontSize: "14px"
    fontWeight: 500
  effects-status:
    fontFamily: "Inter"
    fontSize: "12.5px"
    fontWeight: 500
rounded:
  clip: "3px"
  control: "5px"
  transport: "6px"
  window: "10px"
spacing:
  item-horizontal: "8px"
  item-vertical: "6px"
  button-horizontal: "10px"
  panel-horizontal: "18px"
  header-horizontal: "22px"
components:
  action-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.canvas}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    height: "30px"
  action-secondary:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    height: "30px"
  category-selected:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.text}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    height: "26px"
  sample-tile:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    typography: "{typography.clip}"
    rounded: "{rounded.control}"
    width: "96px"
    height: "46px"
  transport-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.canvas}"
    rounded: "{rounded.transport}"
    width: "42px"
    height: "42px"
  rail-action:
    backgroundColor: "{colors.canvas}"
    textColor: "{colors.text}"
    rounded: "{rounded.control}"
    width: "48px"
    height: "30px"
  stretch-panel:
    width: "640px"
    height: "320px"
  stretch-listen:
    textColor: "{colors.stretch-cream}"
  stretch-save:
    textColor: "{colors.stretch-ink}"
  effects-panel:
    width: "640px"
    height: "352px"
  effects-collapsed:
    width: "640px"
    height: "48px"
  effects-footer:
    backgroundColor: "{colors.effects-body}"
    width: "640px"
    height: "48px"
  effects-knob:
    width: "32px"
    height: "32px"
  effects-toggle:
    width: "24px"
    height: "24px"
  effects-value:
    textColor: "{colors.effects-cream}"
    typography: "{typography.effects-value}"
    width: "56px"
    height: "16px"
  effects-listen:
    textColor: "{colors.effects-cream}"
    typography: "{typography.button}"
  effects-save:
    textColor: "{colors.effects-ink}"
    typography: "{typography.button}"
  effects-reset:
    textColor: "{colors.effects-cream}"
    typography: "{typography.button}"
---

# Design System: MixJam Techno

## Overview

**Design goal: "A clear live-set score"**

The arrangement is the main work area. Graphite surfaces separate clip names,
track numbers, and playback controls. Lime marks primary actions. Category
colors identify the same sounds in the browser and arrangement.

The owner selected this design for the classic loop sequencer. It uses imported
Syncopate branding and titles, Inter Medium interface text, flat controls,
and consistent spacing.
Time Stretcher and Effect Studio are the owner's scoped exceptions. Time
Stretcher uses compact teal textured metal, cream labels, dark displays, rotary
controls, and lamped Listen and Save buttons. Effect Studio uses a compact red
metal landscape panel with small rotary controls, chrome section switches,
green section lamps, and black and ivory action buttons. These treatments do
not change the shared studio theme.
RaveFold supplies artistic references only. Product behavior remains governed by
`PRODUCT.md` and `COMPATIBILITY.md`.

**Key Characteristics:**

- Graphite surfaces with off-white text and restrained borders.
- Syncopate branding and titles in uppercase; Inter Medium for all other text.
- A wide arrangement with transport above and sound discovery below.
- Lime primary actions, category-colored clips, and coral recording signals.

The implementation is native Windows Rust with eframe/egui. Tokens and painting
live in `src/ui/view.rs`, with the Time Stretcher panel in `src/ui/stretch.rs`
and Effect Studio in `src/ui/effects.rs`;
native dialogs and input adapters live in `src/ui.rs`. Application
state and behavior live in `src/application.rs`. Sizes here use portable `px`
labels for egui logical points. The sidecar records native component metadata;
it does not define browser components or application behavior.

## Colors

### Primary

Accent lime identifies Play/Pause, primary empty-state actions,
the playhead, enabled routing controls, and focus or active-state outlines.
Coral identifies recording, errors, and selected synth notes.

### Secondary

Loop, Drum, Bass, Sequence, Sphere/Wave, Effect, Voice, and Other Sample colors
identify sounds. Tiles use a 20% category-color overlay; arrangement clips use the
full category fill with canvas-colored text. Selection adds a text-colored clip border.

**The Shared Category Rule.** Use the same category color for a sound in the
browser and arrangement. Pair color with its name or category label.

### Neutral

Canvas is the deepest structural surface. Panel supports transport, browser,
mixer, and floating tools. Raised and Hover distinguish controls and interaction
states. Text carries primary labels; Muted carries counts, metadata, and status.
Line separates surfaces. Selection and Warning retain their native widget roles.

Stretch Cream and Stretch Ink apply only to Time Stretcher. Cream carries its
title, labels, values, and Listen text. Ink carries Save text on the ivory
button. The panel's teal, black, ivory, amber, and green material colors come
from the prepared skin; they do not replace shared palette tokens.
Stretch Body continues the skin's outer bezel under the status footer, with
Stretch Body Edge as its one-point outline. The Stretch Well colors fill the
status footer, the source list, and the Target menu: Well and Well Line for
the surface and border, Well Deep for the search field, Well Hover and Well
Selected for rows. Stretch Error tints error text and its mark. Stretch Lamp
Rim outlines both lamps.

Effects Cream carries Effect Studio's live title, labels, values, pointers, and
black-button lettering. Effects Ink carries Save sound lettering on ivory.
Effects Body continues the bezel below the artwork. Effects Well and Well Line
form its status display; Effects Error marks local error text. Red metal,
chrome, and static knob colors come from the prepared artwork. The section
lamps show the live enabled state and do not replace named switch states.

## Typography

The native build embeds Syncopate Bold 700 for branding and titles, and
Inter Medium 500 for all other application text. Render branding and every
project, section, window, modal, and empty-state title in uppercase Latin text.
Transliterate special letters to their Latin counterparts before uppercase
layout, including Ș/Ş to S, Ț/Ţ to T, Â/Ă to A, and Î to I. Keep stored
project titles and other user data in their original case. Window IDs retain
their original identity independently of visible title formatting.

Syncopate is unchanged from the pinned Google Fonts source. Title letters use
their Latin counterparts instead of a Romanian fallback face. Inter is the
unchanged static text face from the official 4.1 release. Both families include
ASCII.
Egui's bundled proportional fonts follow the primary families for remaining
missing glyphs. Keep Syncopate's Apache 2.0 license, Inter's OFL license, and
provenance with the application; see `assets/fonts/README.md`.

Display is the MixJam name. Headline is the standard native heading. Section
labels identify Arrangement, Sounds, and Mixer. Title names the project. Body
serves fields and explanatory text. Body, Button, Clip, Small, and Metadata all
use Inter Medium 500. Track labels and numeric controls also use Inter. Clip labels use
the clip size, reduced to metadata size when their rectangle is below 28 points
high. Small and Metadata support compact controls and sample details.

The bar-and-beat readout uses Inter Medium at 23 points in a fixed
112 by 42-point container. Changes to digit widths or bar count must not move
the adjacent transport controls. Status text
uses Inter Medium at 12 points. These are local sizes within the same text family.

Time Stretcher uses a live uppercase Syncopate title at 40 × (640/880), about
29.09 logical points at the supported display scales. Its other text uses Inter
Medium. Text is rendered over the skin, so source names, BPM values, warnings,
and control labels stay current.

Effect Studio uses a live uppercase Syncopate title through Effects Title.
Inter labels use the shared Clip size; source and action text use Body and
Button. Effects Value and Effects Status support its numeric wells and footer.
Keep this standard desktop text scale when preparing hardware artwork.

**The Name Remains Rule.** Keep sample names visible on clips. Use one line below
38 points of clip height and up to two lines at larger heights. Truncate long names and
retain the full name in a tooltip.

## Layout

The native client starts at 1440 by 1000 logical points, with a minimum of 960 by
720. The header is 66 points high. Transport occupies 68 points above the
arrangement. A fixed 64-point tool rail sits at the right. The bottom Sounds/Mixer
panel starts at 28% of the content height, clamped to 220-290 points. Drag its
top divider to resize it. A centered grip marks the divider. The panel retains
its height for the current session, with a minimum of 180 points for Sounds and
310 points for Mixer. Opening Mixer raises a shorter panel to that minimum.
The maximum height leaves 220 points for the arrangement plus 68 points for
transport. Status
occupies 30 points below it. Its application version, prefixed with `v`, stays at the right inner
margin, with space reserved before status text is truncated. Use 12-point Inter
Medium and the theme's hyperlink color. Hover and keyboard focus show the link
underline. The version opens the GitHub repository in the system browser.
Sounds and Mixer share this panel size.

The arrangement shows occupied tracks and one drop target for a new track, while
the model retains 64 slots. Original track numbers remain stable. Rows are
34-48 points high and scroll vertically. The label column is 74 points wide;
the fixed ruler is 26 points high. Bars use 62 points multiplied by zoom. The
timeline scrolls horizontally without moving track labels.

Follow is enabled by default in the transport. It moves the horizontal viewport
when the playhead approaches an edge, including seeks and backward loop jumps.
It leaves vertical scrolling unchanged. While paused, manual horizontal scroll
stays where the user leaves it until the position or viewport geometry changes.
Switch Follow off to keep horizontal scrolling under manual control during
playback. The toggle is view state for the current session, not a project edit.

Sound search and the product, category, and sort menus sit above a horizontally
scrolling category row. Tiles wrap at the available width and scroll vertically.
This keeps category access separate from palette height. A tile's width follows
its bar count: 96 points per bar plus the 8-point gaps it spans, so a two-bar
tile covers two one-bar tiles. A sound longer than a row fills the row. The sort
menu orders the filtered sounds by library order, name, length, BPM, or
category; it is view state for the current session.

Mixer strips appear only for occupied tracks. Track and Master strips are
124 points wide. Master stays outside the horizontal track scroller, whose
scrollbar remains visible. Track numbers, sound summaries, signed gain values,
pan descriptions, and output state stay grouped within each strip. A track with
several distinct sounds shows their count and lists their names on hover.
Faders are at least 74 points high at the minimum window size.

Step synth constrains its floating window to the viewport and scrolls vertically.
Sample libraries uses the same viewport constraint and vertical scrolling. A
large expanded Folder tempos list must leave the provider controls reachable.
Its note grid uses 33 by 16-point cells with 2-point gaps; this compact density is
scoped to the grid. Other tool controls retain the shared control spacing.

Time Stretcher uses a fixed 2:1 panel, 640 by 320 logical points at the supported
100%, 150%, and 200% display scales. The source selector spans the panel above
the paired Original BPM and Target BPM controls. Listen and Save occupy the
bottom row. Collapse uses separate 640 by 64-point artwork with a complete lower
bezel. This compact hardware layout is local to the tool.

Effect Studio's current compact implementation uses a 640 by 352-point
landscape skin above a 48-point footer.
The complete expanded window is 640 by 400 points. The separate collapsed skin
is 640 by 48 points. Its source name sits above two rows of inset banks: Filter
and Echo above Robotize and Distortion / Output. Keep the small knob artwork
and standard text scale; do not enlarge the panel into a portrait surface.
These dimensions apply at the prepared display scales. At other scales the
nearest artwork density determines the logical size, preserving physical pixels.

This is a resizable Windows desktop surface. No browser, mobile, touch, or
high-DPI acceptance is implied by these logical dimensions. Native capture cases
are defined by `tools/verify-ui.ps1`; execution results belong in evidence reports.

## Elevation & Depth

Flat color layers and thin borders separate controls from the work area. The
shared studio has no custom gradient, metal bevel, waveform decoration, or shadow
scale. Shared floating windows use the default shadow from the pinned egui dark theme.
Time Stretcher and Effect Studio use textured metal, inset displays, knobs,
and button depth from their prepared PNG skins. Hover, focus, pressed, and
disabled feedback remains native painting over each skin.
Hover and focus states change immediately. The application has no motion tokens.

## Shapes

Clips use the smallest corner radius. Controls and sound tiles share the control
radius; transport has a slightly larger radius. Floating windows use the window
radius. Resting native widgets use one-point borders. Selected clips use a
two-point text-colored border; active rail actions use a one-point accent outline.
Transport keyboard focus uses a two-point inset outline: Canvas on the bright
Play/Pause button and Accent on secondary controls.

## Components

### Actions and transport

Primary action buttons use Accent with Canvas text. Secondary buttons use Raised
with Text. Undo and Redo use transparent PNG arrow icons from the shared artwork
cache, with action names in tooltips and accessibility labels.
Help uses the shared PNG library icon with Help as its tooltip and
accessibility label.
Export WAV is available through the Export icon in the right-hand tool rail.
Play/Pause stays prominent in the transport; other transport controls
use raised surfaces and hover fills. Record uses a coral circle with a white ring
during capture. Symbols are painted native geometry with tooltips and accessible
action names.

### Tool rail

The header places a 40-point tribal mask before the MixJam Techno title.
The artwork retains its original colors and transparent background. The native
window, taskbar, and Windows executable use the same preserved mask.
`assets/app/` contains its PNG and multiple-size ICO. The viewport embeds
the PNG; `build.rs` links the ICO into the executable.

Transparent PNG artwork supplies alpha masks. `src/ui_icons.rs` retains
their alpha and normalizes RGB to white; the view applies text, muted, or accent
tints. Textures are cached per egui context. Action names are available through
tooltips and accessibility labels. Recording destination remains a visible
track number. Canonical PNG artwork and provenance remain in `assets/rail/`.
SVG files and embedded SVG artwork are not allowed in tracked or non-ignored
project files. Ignored `local-data/` and `tmp/` files are outside this check.
Original preservation material stays in `local-data/` under the owner's layout.

Control icons use prepared 20-pixel PNGs and the title mask uses a 40-pixel PNG
at 100% Windows scaling. Three display variants cover 100%, 150%, and 200%:
20/30/40-pixel controls and 40/60/80-pixel title artwork.
Each image texel maps to exactly one physical screen pixel, with
pixel-aligned bounds and no runtime resizing. At other scale factors, select
the nearest prepared size; its logical size can differ slightly. Reload cached
textures when the required density changes. Windows ICO packaging is generated
from the canonical PNG for the shell icon.

### Sounds and clips

Sound tiles show the name above the category label and bar count. A tile too
narrow for both keeps the bar count. A 20% overlay
of the category color tints the whole tile; there is no category dot.
Hover or selection uses Hover; selection adds a category-colored border.
One tooltip groups the full name, product, category, code, sample IDs, source
tempo, nominal bars, actual audio duration, frames, rate, channels, and WAV path
when available. Nominal bars and audio duration remain separate facts.
Arrangement clips use flat category fills and names. They do not display a
waveform. A selected clip can show a gain line; it is a control indicator.

### Mixer and tool windows

Mixer shares the graphite shell and type hierarchy. Track identity and gain lead
the hierarchy; L/R routing uses compact checkboxes with a check mark and named
enabled state. Both outputs off shows Muted. Gain is a signed multiplier with
values rounded to one decimal place, silence and unity marks, an exact-entry
field, and a Unity reset. Both track and Master values use this format, including
gain tooltips. Only the active entry field shows full precision. Display rounding
and focus without an edit must not change the saved gain or audio.
Negative values show Inverted. The header and Help explain polarity inversion.
Pan has a readable left/center/right value and a Center reset. Every control has
a track-specific accessible name; Master has its own gain labels.
Arrangement and mixer L/R labels use Accent when enabled and Muted when off.
Both expose the same checked state and a track-specific on/off tooltip. Lane
controls show keyboard focus. A lane edit requests a repaint so the already
drawn mixer refreshes even if the pointer stops. Pan remains independent of L/R.
The sample-conflict modal confirms only through Keep choices, because
confirmation is an undoable project change; Esc hides it without an edit and
the status bar reopens it.
The idle status reads Ready. Errors and application notices remain visible.
The empty mixer has one instruction for adding a sound to the arrangement.
Floating libraries, sample editing, and Step synth use the shared native widget
theme. Time Stretcher and Effect Studio use their scoped hardware panels below.
The frontend submits
edits to the application layer; visual changes must preserve those commands.

### Time Stretcher

`src/ui/stretch.rs` paints `assets/stretch/{100,150,200}/panel.png`, prepared at
640 by 320, 960 by 480, and 1280 by 640 pixels. Select the nearest density and
paint one texel per physical pixel with aligned bounds, as for shared artwork.
The skin supplies the material finish and static hardware forms. The title,
source selector, values, labels, knob pointers, and input states remain live.
Upper corner screws are replaced by native controls: collapse at the left and
Close at the right. Help uses a cream question-mark icon beside Close. The
collapsed state uses a separate `collapsed.png` skin with a complete lower rim,
prepared at 640 by 64, 960 by 96, and 1280 by 128 pixels. It retains the title,
expand, and Close controls. It does not clip the expanded skin.
Black Listen and ivory Save buttons pair their action labels with amber and
green lamps; lamp color does not replace an action name.
Center the visible action lettering and its shadow horizontally and vertically
on each flat button face. Font line-box bounds and the surrounding bevel are
not the visible face bounds. A label too long to clear its lamp moves right
only until 10 by 640/880 points separate it from the lamp's rim.
The hardware artwork retains its 640 by 320-point size. The expanded window is
640 by 400 points and reads as one unit. A Stretch Body base continues the
bezel below the artwork, with bottom corners that match the skin's radius, and
leaves no transparent area inside the window. The status footer is an inset
display well in that base, not a separate floating container.
Short status and error messages are centered within the footer's text area.
The message and Import details control share the footer's vertical center.
Long text wraps inside that footer and scrolls when needed; it does
not enter the control composition. Missing audio's Import details control sits
inside the same footer beside its message. Collapsing hides the footer.

Original BPM supports 30-300; Target BPM offers 120, 140, 160, and 180.
Knobs support pointer and keyboard input. Drags accumulate unrounded movement
and hold when the pointer stops. Shift changes only new movement for fine
adjustments. Hover does not change either BPM. Each wheel event adjusts one
step; later frames do not repeat it. Double-click Original to restore the
selected source tempo; double-click Target to reset to 140 BPM. Target's four
positions span its full 270-degree arc and have distinct major marks.
Original turns one degree per BPM across the same arc. A Target drag holds
at most 10 BPM of overshoot past either end, so reversing takes effect at once.
The source selector truncates long names, retains full-name tooltips, and opens
a searchable list with bounded, virtualized rows. Each opening starts with an
empty, focused search. The list and the Target menu use the well material and
20 by 640/880-point text, not the shared widget theme.
Tab reaches the source, values, knobs, and actions before Help, collapse, and
close. Keyboard focus is a solid cream ring; pointer hover is a faint one-point
ring. Help has a cream circular outline at rest so it reads as a control.
Esc closes the tool only when it is the top window and nothing inside it is
in use. An open list or menu, a value being typed, a modal, or a window above
it takes Esc first.

Listen and Save require an available source. Processing runs in the application
worker and disables repeated requests. Listen becomes Stop listening while this
tool's preview is active. The amber lamp lights only during processing or that
preview; green confirms a successful save and clears after settings change.
Cancelled or failed saves do not light green. Each lamp sits in a dark bezel
with a metal rim, so it is located on the black face and a lit lamp reads on
the ivory one. Unlit lamps stay visibly dimmer than lit ones. An unavailable
button fades its lettering with its face.
A successful save is a receipt. The tool keeps its source and both tempos,
names the saved file and its category in the footer, and leaves the full path
in the application status bar.
The bottom status well shows local progress, errors, save confirmation, or the
sound and tempo values of a running preview. Changed drafts are distinguished
from that preview. When no preview runs and no message is pending, show the
duration change and approximate pitch-preservation warning; add Listen before
saving until the current settings have been heard. Equal tempos state that the
duration is unchanged. Errors begin with Cannot, carry a drawn mark, and are
announced as errors; color is not their only signal. The status is a polite
live region for assistive technology. Feedback
also retains its full text in a tooltip. The panel's Help opens the audible
limitations, ranges, input, reset, and lamp meanings directly in a native window.
Missing audio exposes Import details and a concise warning. Esc and the
close control close the tool; the title triangle collapses or expands it.
The form sends selections and action requests through `src/ui.rs` to the
existing application operations. It does not process audio or write files.

### Effect Studio

`src/ui/effects.rs` paints five prepared assets at each of the three display
densities: `panel.png`, `collapsed.png`, `toggle-off.png`, `toggle-on.png`, and
`knob.png`.
The separate collapsed skin has a complete bezel. Select the nearest density,
align image bounds to physical pixels, and paint one texel per physical pixel.
See `assets/effects/README.md` for dimensions and generation provenance.

The red metal skin supplies static material, empty bank recesses, and button
faces. Separate knob sprites supply small black bodies. Keep title, source name,
section labels, ticks, numeric wells, values, dial pointers, lamps, header
controls, and interaction feedback live. Collapse is at
the upper left; Close is at the upper right; Help uses a drawn icon beside Close.
Help is available while expanded. Esc first leaves exact entry or a window or
modal above the tool. Otherwise it closes the topmost Effect Studio window.

Filter has Cutoff, Resonance, Overdrive, and Filter volume. Echo has Delay
(beats), Feedback, and Echo volume. Robotize has Color and Robotize volume.
Distortion / Output has Dirty distortion, Noise distortion, and Output volume.
Knob artwork is 32 points square at the prepared scales, with 42-point input
regions. Section switch sprites are 24 points square. The three optional banks
pair a named on/off switch with a live green lamp. Disabled banks dim their
labels and controls. Turning a bank off discards its settings; turning it on
uses that bank's defaults. Reset restores the complete existing effects draft.

Keep knobs, values, and captions in separate vertical bands. Current upper-row
centers are 117, 144, and 163 points; lower-row centers are 216, 244, and 263.
Lower-bank section captions sit at 185 points. Check rendered glyph bounds
against dial arcs, numeric wells, and adjacent controls at each prepared scale.
Change positions to restore clearance while retaining small desktop controls.
The owner rejected crowded labels; these coordinates record the current
implementation and do not establish visual acceptance. Native capture review
remains pending for this revision.

Knobs accept drag, wheel, arrow keys, Home, End, and AccessKit numeric actions.
Drags accumulate unrounded movement and hold when the pointer stops. Shift
applies fine adjustment to new movement. Wheel events are consumed once.
Double-click resets a knob; clicking its numeric well permits exact entry.
Cutoff and Color retain logarithmic knob mapping; each wheel, arrow, or
accessible step moves them 1% of the dial. All parameter ranges,
defaults, processing algorithms, and Step synth's shared effects form remain
unchanged. Accessible controls expose names, current values, ranges, and state.
The source name is a live truncated label with a full-name tooltip and its full
text as the accessible value.

Listen and Save sound require selected, available audio. Missing audio offers
Import details in the footer. Processing and preview retain the existing
synchronous application calls; the panel does not add Time Stretcher's worker
or Stop listening behavior. Save sound writes a WAV to Effect and selects that
saved sound at the source BPM. The local footer shows preview feedback, reset,
save choice, cancellation, errors, and save confirmation. It is an inset display
inside the continued bezel. Center short messages; wrap and scroll long text.
Reserve room for Import details beside missing-audio feedback. Status text has
a full tooltip and a polite accessible live region; errors include an Error
label. No original-product DSP equivalence is established by this presentation.

### Empty states and feedback

The empty sound browser offers Choose WAV folder. Filtered empty
results offer Clear filters. The status strip truncates long messages and gives
the full text on hover. Import details remain available when a project carries
compatibility notices.

Keep task guidance in Help and control tooltips. Do not repeat drag, preview,
tempo, or save instructions across the workspace and tool windows. Keep concise
warnings for missing audio, unsaved recordings, polarity inversion, and approximate
pitch preservation. Import details retain all saved source and placement metadata
as selectable text. Help groups gestures by action and scrolls within the viewport.

Display full filesystem paths without the Windows extended-length prefix.
Show network paths as `\\server\share`. Apply this presentation rule to folder
labels, tooltips, status messages, and import details. Keep stored paths intact;
display formatting must not change file access, project data, or settings.

## Design rules

### Use

- Preserve graphite surfaces, Syncopate branding and titles, Inter Medium
  interface text, and lime primary actions.
- Use the same category and sample names across views.
- Keep track numbers stable and give actions accessible names.
- Check native layouts at the supported minimum size after changes.

### Avoid

- Keep metal hardware treatments scoped to Time Stretcher and Effect Studio.
  Do not add repeated waveform traces.
- Do not use color as the only way to identify a sound or action.
- Do not put application rules in the visual layer.
- Do not use a screenshot as evidence for audio parity, device behavior, or
  assistive technology support.
