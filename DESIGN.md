---
name: MixJam Techno
description: A native cobalt and metallic sequencer with capacity for 64 tracks.
colors:
  cobalt: "#142174"
  navy: "#05092a"
  ice: "#d5e6f7"
  signal-green: "#67eb58"
  record-red: "#f13d36"
  metal-top: "#bed6eb"
  metal-bottom: "#5d7faa"
  metal-hover: "#eaf6ff"
  metal-edge: "#3e5689"
  category-top: "#89b6e0"
  category-bottom: "#2b5599"
  category-selected: "#f9dc7c"
  label-backplate: "#1c3773"
  loop: "#f4da84"
  drum: "#e5b485"
  bass: "#b8ddb8"
  sequence: "#b4d3ef"
  sphere: "#b2daee"
  effect: "#eba4a3"
  voice: "#d9b8de"
  other-sample: "#e7c398"
  clip-ink: "#141925"
  window: "#0f194b"
  widget-rest: "#283a77"
  widget-hover: "#415ea6"
  widget-active: "#4e71ba"
  widget-selection: "#3459b1"
typography:
  display:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "31px"
  headline:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "20px"
  title:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "15px"
  body:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "13px"
  label:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "14px"
  clip:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "11px"
  clip-dense:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "9px"
  status:
    fontFamily: "Ubuntu Light, sans-serif"
    fontSize: "12px"
rounded:
  square: "0px"
  drag-ghost: "4px"
spacing:
  item-horizontal: "7px"
  item-vertical: "5px"
  sample-grid: "6px"
  inset: "8px"
  panel: "10px"
components:
  metal-button:
    backgroundColor: "{colors.metal-top}"
    textColor: "{colors.navy}"
    typography: "{typography.body}"
    rounded: "{rounded.square}"
    height: "29px"
  metal-button-hover:
    backgroundColor: "{colors.metal-hover}"
  category-button:
    backgroundColor: "{colors.category-top}"
    textColor: "#ffffff"
    typography: "{typography.label}"
    rounded: "{rounded.square}"
    width: "90px"
    height: "30px"
  sample-tile:
    backgroundColor: "{colors.loop}"
    textColor: "{colors.clip-ink}"
    typography: "{typography.clip}"
    rounded: "{rounded.square}"
    width: "118px"
    height: "39px"
  transport-button:
    backgroundColor: "{colors.metal-top}"
    textColor: "{colors.navy}"
    rounded: "{rounded.square}"
    width: "48px"
    height: "39px"
  sound-search:
    backgroundColor: "{colors.navy}"
    textColor: "{colors.ice}"
    typography: "{typography.body}"
    width: "180px"
---

# Design System: MixJam Techno

## Overview

RaveFold is an artistic reference only. Its icons, artwork, typography, colors,
and visual composition can guide this interface. Its features and specifications
do not define this application's behavior. Use the product plan for feature scope
and measured compatibility evidence for parity decisions.

**Creative North Star: "The Cobalt Sequencer Console"**

The interface uses a cobalt work area, metallic controls, and colored sample strips. It is a dense music console: the arrangement carries the visual weight, while transport and editing controls remain close to the work. Labels, track numbers, and familiar playback symbols make the compact layout usable.

Metallic surfaces use narrow tonal bands and a highlight near their top. Sample blocks use soft category colors, dark names, and a fine repeating trace. These materials keep the reference application's visual identity visible in the native Rust implementation.

**Key Characteristics:**

- Deep cobalt panels and near-black track wells.
- Square, beveled metal controls with compact labels.
- Colored sample blocks that retain their names.
- Green output and active-state signals; red recording and play-position signals.

This document records the shared native source in `src/ui.rs` and `src/main.rs`. The older BMP captures in `evidence/ui/` record the historical 16-track single-profile build. Eight fresh captures in `evidence/ui/merge/` record the 64-track build, compact rows, icon rail, mixer, and library dialog at 1440 by 1000, 1024 by 768, and the minimum 960 by 720. Sizes in the source are egui logical points; frontmatter uses portable `px` labels for the same numeric values. The platform is native Windows with eframe/egui. Impeccable's platform schema has no Windows value. HTML/CSS snippets in the sidecar are design-panel representations, not application components or a web implementation. Sidecar tonal ramps are generated swatch aids; the native UI uses the source-backed frontmatter colors rather than those ramps.

## Colors

The palette places pale metal and sample colors above a saturated blue shell.

### Primary

- **Cobalt:** panel surfaces and the surrounding workspace.
- **Navy:** track wells, recessed readouts, and dark text on metal.
- **Ice:** normal text on blue surfaces.

### Secondary

- **Metal Top, Metal Bottom, and Metal Hover:** the reusable metal button material and its hover highlight.
- **Category Top and Category Bottom:** the stronger blue material for sample-group navigation.
- **Category Selected:** the selected sample group's border.
- **Label Backplate:** the dark inset behind category labels and track numbers.

### Tertiary

- **Loop, Drum, Bass, Sequence, Sphere, Effect, Voice, and Other Sample:** sample-group fills. The same mapping applies to palette tiles and arrangement clips.
- **Signal Green:** enabled stereo outputs, active metal controls, and focus borders on metal and transport buttons.
- **Record Red:** record symbols, the armed-track dot, the current bar, and the play-position line.

### Neutral

- **Clip Ink:** readable labels on pale sample fills.
- **Metal Edge:** the resting border of standard metal buttons.
- **Window:** floating editor windows.
- **Widget Rest, Widget Hover, Widget Active, and Widget Selection:** the egui control state fills used by sliders, fields, and standard controls.

**The Shared Sample Color Rule.** Use the same category fill in the palette and the arrangement so the sound remains recognizable during placement.

## Typography

The implementation uses egui's bundled proportional family. The resolved primary face is `Ubuntu-Light`; the bundled font definitions provide symbol fallbacks. It does not load a system font or a separate display face.

- **Display:** the italic, strong MixJam wordmark.
- **Headline:** the strong Techno label and the italic mixer heading.
- **Title:** the project name, with an asterisk when it has unsaved changes.
- **Body:** normal controls, file actions, and ruler labels.
- **Label:** sample-group buttons.
- **Clip:** sample names, inset from the upper-left corner. Names wrap to two lines and use an ellipsis when needed.
- **Clip Dense:** sample names when a clip is less than 24 logical points high. These use a single line with an ellipsis when needed; hover gives the full name.
- **Status:** the bottom status line and drag feedback.

Track numbers use a larger proportional size (17 logical points). The bar-and-beat readout uses strong proportional text (21 logical points). The large, faint MixJam mark in the arrangement is background material, not a heading.

White category labels and track numbers sit on dark backplates so the metallic highlights do not cross their letterforms.

**The Name Remains Rule.** Keep the sample name visible on a colored block. The trace supplements the name; it does not replace it.

## Layout

The native window starts at 1440 by 1000 logical points and has a minimum client size of 960 by 720. The header is 58 points high. A fixed right tool rail is 76 points wide. The arrangement occupies the central area. Below it sit a 70-point transport strip, the sound palette or mixer, and a 28-point status line.

The lower work panel uses 27% of the available content height, limited to 180-290 points. The project has capacity for 64 tracks. The arrangement renders used tracks and omits unused rows. Original track numbers remain the identity of each visible row. Used rows scroll vertically and retain readable heights. The track-label column is 74 points wide; the bar ruler is 26 points high. A bar is 62 points wide at normal zoom, multiplied by the selected zoom. The timeline scrolls horizontally. Track labels stay at the left of the viewport; the bar ruler stays above the scrolling rows.

General control spacing uses the frontmatter's horizontal and vertical steps. Palette tiles use the sample-grid step in both directions. Sample-group buttons sit in two 92-point side columns. The center grid changes its column count with the available desktop width and scrolls vertically. The search field has a desired width of 180 points. Product and category menus filter all registered libraries; category choices come from the current catalog.

Used mixer tracks use 64-point strips with gain, pan, and L/R routing; unused tracks do not render strips. Master uses a 66-point strip. The mixer scrolls horizontally when needed. The Step synth window limits its height to the content area minus 80 points, with a 240-point lower bound, and scrolls vertically to keep its lower controls reachable.

The supported adaptation is a resizable desktop console. Fresh captures cover
1440 by 1000, 1024 by 768, and 960 by 720 pixels. They show used track 64,
empty/sparse projects, omitted unused mixer strips, and the complete icon rail.
Eighteen UI tests cover compact input mapping, scrolling, tempo, pan, filters,
tooltips, accessible action names, and cached textures. These checks do not
establish touch, mobile, browser, high-DPI, or native screen-reader behavior.
At 960 by 720 the final palette shortcut row is clipped; the category menu
still exposes those groups. This remains a small layout follow-up.

## Elevation & Depth

The signature depth comes from painted metal bands and thin borders. `metallic` draws 24 horizontal bands with a brighter region near 27% of the control height. The header, ruler, track labels, transport controls, category buttons, rail, and mixer use this material with different endpoints. Track wells remain dark. Native egui windows retain the library's window treatment; the application defines no custom shadow scale.

**The Metal Is Functional Rule.** Apply the metal material to controls and structural chrome. Keep sample blocks flat enough for their labels and category colors to remain clear.

## Shapes

Custom buttons, category tabs, track rows, sample tiles, clips, and readouts use square corners. Standard egui fields and windows retain the library's default corner treatment. The pointer's drag-feedback label uses the small rounded token.

Resting metal borders are one point wide. Active standard metal buttons use a two-point green border. Selected category buttons use a two-point warm border. Selected clips use a two-point white border; resting clips use a one-point muted border.

## Components

### Metal buttons

Compact, square controls with a light top, darker base, and navy text. Text-button width follows measured glyph width plus 18 points, with a 51-point minimum, bounded by the available width. Hover brightens the top of the material. Active state and keyboard focus use green outlines.

The narrow right-hand rail uses embedded PNG icons rendered from SVG source
under `assets/rail/`. Action names appear only in tooltips and accessibility
labels. Numeric recording destinations remain visible. The icons use the
preserved RaveFold stroke family and matching extensions, with navy strokes
on transparent backgrounds. Icon source and provenance remain with the assets.

### Sample-group navigation

Blue metal buttons display white centered labels on dark square backplates (80 by 21 logical points). The selected group has a warm border. Ten familiar group shortcuts occupy the two columns beside the sound grid. The category menu also exposes the dynamic groups found in the registered catalogs, and the product menu can select one library or all libraries. The current source does not give these custom category buttons a separate hover or focus treatment.

### Sample tiles and arrangement clips

Pale, square strips use the shared group colors and dark labels. A thin, repeating white trace runs behind the name. The trace is a visual motif, not measured audio amplitude. Palette hover and selection share a white outline. Arrangement selection adds a gain line inside the clip. Dense track rows use single-line names, while taller blocks can show two lines. Arrangement hover gives the full sample name. The tile tooltip also gives bar length, sample rate, and placement instructions.

### Transport

Metal icon buttons sit beside the dark bar-and-beat readout. Symbols are drawn directly with egui geometry. Playback and loop state can use green symbols. Record remains red, with a white ring during recording. Tooltips name the actions and the Space shortcut.

### Search and standard fields

The sound search uses a recessed navy egui text field with ice text and the hint "Find a sound". Project tempo uses a menu with 120, 140, 160, and 180 BPM. Source tempo fields retain factual values separately. Zoom, numeric editors, and floating tool windows use standard egui widgets with the shared blue state fills. Their detailed geometry remains defined by the pinned egui version.

### Routing and mixer

Paired speaker symbols flank each track number and its dark backplate. Enabled output is green; disabled output is blue. Mixer strips pair L/R toggles with vertical gain faders and horizontal pan controls. The pale metal mixer background and navy labels distinguish level control from the dark arrangement. The lower work panel switches between mixer and sound palette. There is no project playback-pitch fader; pitch changes remain available in the sample editor.

### Status

The bottom strip gives task feedback in ice text on a dark field. Errors use pale coral text. Hover exposes the full message when the compact line cannot show it all. Imported tempo changes have an Import details action; source relocation notices remain visible in the task message and are saved with the project.

## Do's and Don'ts

### Do:

- **Do** preserve the cobalt console, metallic controls, and labeled colored clips from the supplied references.
- **Do** reuse the sample-group color mapping in every sample representation.
- **Do** keep used tracks reachable through vertical scrolling, with stable original track numbers and a fixed bar-ruler strip.
- **Do** omit unused arrangement rows and mixer strips while retaining the project's 64-track capacity.
- **Do** distinguish selected, armed, enabled, and recording states with the established borders, dots, and symbols.

### Don't:

- **Don't** replace the reference's console with a new visual identity.
- **Don't** present the repeating clip trace as measured waveform data.
- **Don't** treat design-panel HTML previews as shipped native components.
- **Don't** infer a mobile or web layout contract from these desktop captures.
