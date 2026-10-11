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
typography:
  display:
    fontFamily: "Space Grotesk"
    fontSize: "27px"
    fontWeight: 700
  headline:
    fontFamily: "Space Grotesk"
    fontSize: "20px"
    fontWeight: 700
  section:
    fontFamily: "Space Grotesk"
    fontSize: "18px"
    fontWeight: 700
  title:
    fontFamily: "Space Grotesk"
    fontSize: "15px"
    fontWeight: 700
  body:
    fontFamily: "Space Grotesk"
    fontSize: "13px"
    fontWeight: 400
  button:
    fontFamily: "Space Grotesk"
    fontSize: "13px"
    fontWeight: 500
  clip:
    fontFamily: "Space Grotesk"
    fontSize: "12px"
    fontWeight: 400
  small:
    fontFamily: "Space Grotesk"
    fontSize: "11px"
    fontWeight: 400
  metadata:
    fontFamily: "Space Grotesk"
    fontSize: "10px"
    fontWeight: 400
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
    width: "148px"
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
---

# Design System: MixJam Techno

## Overview

**Design goal: "A clear live-set score"**

The arrangement is the main work area. Graphite surfaces separate clip names,
track numbers, and playback controls. Lime marks primary actions. Category
colors identify the same sounds in the browser and arrangement.

The owner selected this design for the classic loop sequencer. It uses imported
Space Grotesk, flat controls, and consistent spacing.
RaveFold supplies artistic references only. Product behavior remains governed by
`PRODUCT.md` and `COMPATIBILITY.md`.

**Key Characteristics:**

- Graphite surfaces with off-white text and restrained borders.
- Space Grotesk headings, controls, and sample labels.
- A wide arrangement with transport above and sound discovery below.
- Lime primary actions, category-colored clips, and coral recording signals.

The implementation is native Windows Rust with eframe/egui. Tokens and painting
live in `src/ui/view.rs`; native dialogs and input live in `src/ui.rs`. Application
state and behavior live in `src/application.rs`. Sizes here use portable `px`
labels for egui logical points. The sidecar records native component metadata;
it does not define browser components or application behavior.

## Colors

### Primary

Accent lime identifies Play/Pause, Export WAV, primary empty-state actions,
the playhead, enabled routing controls, and focus or active-state outlines.
Coral identifies recording, errors, and selected synth notes.

### Secondary

Loop, Drum, Bass, Sequence, Sphere/Wave, Effect, Voice, and Other Sample colors
identify sounds. Tiles use a small category dot; arrangement clips use the full
category fill with canvas-colored text. Selection adds a text-colored clip border.

**The Shared Category Rule.** Use the same category color for a sound in the
browser and arrangement. Pair color with its name or category label.

### Neutral

Canvas is the deepest structural surface. Panel supports transport, browser,
mixer, and floating tools. Raised and Hover distinguish controls and interaction
states. Text carries primary labels; Muted carries counts, metadata, and status.
Line separates surfaces. Selection and Warning retain their native widget roles.

## Typography

The native build embeds Space Grotesk Regular 400, Medium 500, and Bold 700 from
`assets/fonts/`. Each face includes Latin, extended Latin, and Vietnamese subsets,
followed by egui's bundled proportional fallbacks for missing glyphs. The assets
are derived offline from the preserved WOFF2 package. Keep their OFL license and
provenance with the application; see `assets/fonts/README.md`.

Display is the MixJam name. Headline is the standard native heading. Section
labels identify Arrangement, Sounds, and Mixer. Title names the project. Body
serves fields and explanatory text; Button uses the Medium face. Clip labels use
the clip size, reduced to metadata size when their rectangle is below 28 points
high. Small and Metadata support compact controls and sample details.

The bar-and-beat readout uses strong proportional text at 23 points in a fixed
112 by 42-point container. Changes to digit widths or bar count must not move
the adjacent transport controls. Status text
uses 12 points. These are local roles, not a second font family.

**The Name Remains Rule.** Keep sample names visible on clips. Use one line below
38 points of clip height and up to two lines at larger heights. Truncate long names and
retain the full name in a tooltip.

## Layout

The native client starts at 1440 by 1000 logical points, with a minimum of 960 by
720. The header is 66 points high. Transport occupies 68 points above the
arrangement. A fixed 64-point tool rail sits at the right. The bottom Sounds/Mixer
panel starts at 28% of the content height, clamped to 220-290 points. Drag its
top divider to resize it. A centered grip marks the divider. The panel retains
its height for the current session, with a minimum of 180 points and a maximum
that leaves 220 points for the arrangement plus 68 points for transport. Status
occupies 30 points below it. Sounds and Mixer share this panel size.

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

Sound search and product/category menus sit above a horizontally scrolling
category row. The tile grid chooses its column count from available width and
scrolls vertically. This keeps category access separate from palette height.

Mixer strips appear only for occupied tracks. Track strips are 76 points wide;
Master is 66 points. The horizontal scroller preserves access to every used
track. Gain faders reserve vertical space for routing, track identity, and pan.

Step synth constrains its floating window to the viewport and scrolls vertically.
Its note grid uses 33 by 16-point cells with 2-point gaps; this compact density is
scoped to the grid. Other tool controls retain the shared control spacing.

This is a resizable Windows desktop surface. No browser, mobile, touch, or
high-DPI acceptance is implied by these logical dimensions. Native capture cases
are defined by `tools/verify-ui.ps1`; execution results belong in evidence reports.

## Elevation & Depth

Flat color layers and thin borders separate controls from the work area. The
application has no custom gradient, metal bevel, waveform decoration, or shadow
scale. Floating windows use the default shadow from the pinned egui dark theme.
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
with Text. Play/Pause stays prominent in the transport; other transport controls
use raised surfaces and hover fills. Record uses a coral circle with a white ring
during capture. Symbols are painted native geometry with tooltips and accessible
action names.

### Tool rail

Preserved SVG-derived PNG artwork supplies alpha masks. `src/ui_icons.rs` retains
their alpha and normalizes RGB to white; the view applies text, muted, or accent
tints. Textures are cached per egui context. Action names are available through
tooltips and accessibility labels. Recording destination remains a visible
track number. Source artwork and provenance remain in `assets/rail/`.

### Sounds and clips

Sound tiles show the name above a category dot, category label, and bar count.
Hover or selection uses Hover; selection adds a category-colored border.
Arrangement clips use flat category fills and names. They do not display a
waveform. A selected clip can show a gain line; it is a control indicator.

### Mixer and tool windows

Mixer shares the graphite shell and type hierarchy. L/R routing, track number,
gain, and pan stay grouped. Floating libraries, sample editing, effects, tempo
stretch, and Step synth use the same native widget theme. The frontend submits
edits to the application layer; visual changes must preserve those commands.

### Empty states and feedback

The empty sound browser directs the user to choose a WAV folder. Filtered empty
results offer Clear filters. The status strip truncates long messages and gives
the full text on hover. Import details remain available when a project carries
compatibility notices.

Display full filesystem paths without the Windows extended-length prefix.
Show network paths as `\\server\share`. Apply this presentation rule to folder
labels, tooltips, status messages, and import details. Keep stored paths intact;
display formatting must not change file access, project data, or settings.

## Design rules

### Use

- Preserve the graphite surfaces, Space Grotesk type, and lime primary actions.
- Use the same category and sample names across views.
- Keep track numbers stable and give actions accessible names.
- Check native layouts at the supported minimum size after changes.

### Avoid

- Do not add decorative metal bevels or repeated waveform traces.
- Do not use color as the only way to identify a sound or action.
- Do not put application rules in the visual layer.
- Do not use a screenshot as evidence for audio parity, device behavior, or
  assistive technology support.
