---
name: MixJam Techno
description: A modern native Windows studio for arranging, mixing, and mastering classic sounds.
colors:
  canvas: "#15171a"
  panel: "#1c1f23"
  raised: "#282c31"
  hover: "#34393f"
  line: "#3c4249"
  text: "#eceff1"
  muted: "#a4aeb6"
  accent: "#bee860"
  coral: "#ff8282"
  warning: "#e8c585"
  selection: "#3e5228"
  drag-label: "#f5dc92"
  aero-canvas: "#144084"
  aero-panel: "#0b2450"
  aero-accent: "#96ec5a"
  vintage-canvas: "#0b0d0c"
  vintage-panel: "#141815"
  vintage-accent: "#f89e3e"
  loop: "#d4c182"
  drum: "#d99d78"
  bass: "#a2c485"
  sequence: "#89b0d2"
  sphere-wave: "#7fbeb9"
  effect: "#d99192"
  voice: "#b99ecb"
  other-sample: "#bbb39f"
  modular-utility: "#444f46"
  modular-marine: "#263e42"
  modular-granite: "#404844"
  modular-red: "#922c26"
  modular-olive: "#505e52"
  modular-indigo: "#1a1e1c"
  modular-utility-accent: "#f89e3e"
  modular-marine-accent: "#68ced6"
  modular-granite-accent: "#d8e0d6"
  modular-red-accent: "#ec786a"
  modular-olive-accent: "#f69a3a"
  modular-indigo-accent: "#f69a3a"
  modular-ink: "#0c0e0d"
  modular-cream: "#f8f4e8"
  modular-face: "#181c1a"
  modular-primary: "#daded6"
  modular-danger: "#8a3024"
  meter-ivory: "#f4ecd0"
  meter-green: "#6acc74"
  meter-amber: "#f6b640"
  meter-red: "#ec4434"
  mixer-metal: "#c9c2af"
  mixer-ink: "#1c1d1a"
  mixer-face: "#1d2122"
  mixer-amber: "#f5c156"
typography:
  display:
    fontFamily: "Syncopate"
    fontSize: "20px"
    fontWeight: 700
    lineHeight: 1.041
  headline:
    fontFamily: "IBM Plex Sans"
    fontSize: "20px"
    fontWeight: 600
    lineHeight: 1.041
  section:
    fontFamily: "IBM Plex Sans"
    fontSize: "18px"
    fontWeight: 600
    lineHeight: 1.041
  title:
    fontFamily: "IBM Plex Sans"
    fontSize: "15px"
    fontWeight: 600
    lineHeight: 1.041
  body:
    fontFamily: "Inter"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: 1.21
  button:
    fontFamily: "Inter"
    fontSize: "13px"
    fontWeight: 500
    lineHeight: 1.21
  clip:
    fontFamily: "Inter"
    fontSize: "12px"
    fontWeight: 500
    lineHeight: 1.21
  small:
    fontFamily: "Inter"
    fontSize: "11px"
    fontWeight: 500
    lineHeight: 1.21
  metadata:
    fontFamily: "Inter"
    fontSize: "10px"
    fontWeight: 500
    lineHeight: 1.21
rounded:
  mixer-strip: "1.5px"
  clip: "2px"
  control: "2.5px"
  transport: "3px"
  window: "4px"
  pedal-metal: "9px"
  pedal-plastic: "10px"
spacing:
  item-vertical: "6px"
  item-horizontal: "8px"
  button-horizontal: "10px"
  panel-horizontal: "18px"
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
  category-chip:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.muted}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    height: "22px"
  category-chip-selected:
    backgroundColor: "{colors.hover}"
    textColor: "{colors.text}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    height: "22px"
  sample-row:
    backgroundColor: "{colors.raised}"
    textColor: "{colors.text}"
    typography: "{typography.clip}"
    rounded: "{rounded.control}"
    height: "44px"
  transport-primary:
    backgroundColor: "{colors.accent}"
    textColor: "{colors.canvas}"
    rounded: "{rounded.transport}"
    width: "42px"
    height: "42px"
  rail-action:
    backgroundColor: "{colors.modular-face}"
    textColor: "{colors.modular-cream}"
    rounded: "{rounded.control}"
    width: "48px"
    height: "30px"
  shell-frame:
    backgroundColor: "{colors.modular-utility}"
    textColor: "{colors.modular-cream}"
    typography: "{typography.section}"
    rounded: "{rounded.window}"
  shell-primary:
    backgroundColor: "{colors.modular-primary}"
    textColor: "{colors.modular-ink}"
    typography: "{typography.button}"
    height: "30px"
  shell-danger:
    backgroundColor: "{colors.modular-danger}"
    textColor: "{colors.modular-cream}"
    typography: "{typography.button}"
    height: "30px"
  mastering-pedal:
    backgroundColor: "{colors.modular-face}"
    textColor: "{colors.modular-cream}"
    rounded: "{rounded.pedal-metal}"
    width: "184px"
    height: "228px"
  fx-card:
    backgroundColor: "{colors.modular-marine}"
    textColor: "{colors.modular-cream}"
    rounded: "{rounded.pedal-metal}"
    width: "198px"
    height: "228px"
  mixer-strip:
    backgroundColor: "{colors.mixer-metal}"
    textColor: "{colors.mixer-ink}"
    rounded: "{rounded.mixer-strip}"
    width: "108px"
---

# Design System: MixJam Techno

## Overview

**Creative North Star:** "A clear live-set score"

MixJam Techno is a native Windows desktop studio built in Rust (`eframe`/`egui`
at 100% display scaling) where the multitrack Arrangement commands center stage,
flanked by a docked transport, sound browser, inspector, lower mixing and
mastering boards, and collapsible floating hardware tools. Switching between its
three themes changes palette, surface physics, corner silhouette, and typefaces
in place without moving a widget rectangle, accessible name, or control
behavior.

**Studio** (default) uses flat matte graphite paint and high-visibility lime
accents; **Aero** renders soft Y2K molded plastic shells over deep cobalt and
navy; **Vintage** pairs stark dark rack cabinets with brushed olive-sage
anodized modular plates, corner slotted screws, amber indicators, and bespoke 3D
hardware stompboxes. Procedural painters draw every chassis, dial, and unit
motif in code; only the brand PNG mark, rail masks, and retiring Mixer strip
tiles stay on disk.

**Key Characteristics:**

- Dark studio canvases paired with crisp off-white or warm silk-screen legends
  across all themes.
- Pixel-identical shared layout across Studio, Aero, and Vintage, with material
  chrome isolated inside the central theme registry and shared control painters.
- Six fixed tool palette identities (Utility, Marine, Granite, Red, Olive,
  Indigo) plus bespoke per-unit Mastering and FX enclosures.
- Product scope and compatibility specifications govern application behavior.

## Colors

Each semantic role adapts its exact RGB values per theme in the theme registry
while keeping contrast polarity and functional roles constant across workspace,
rack boards, and floating tools.

### Primary

- **Workspace Accent** (`accent` in Studio, `aero-accent` in Aero,
  `vintage-accent` in Vintage): Marks Play/Pause, primary empty-state actions,
  the playhead, active routing switches, and keyboard focus outlines.
- **Signal Coral** (`coral`): Marks recording arm/capture, validation errors,
  missing source states, and selected Step synth notes.

### Secondary

- **Category Identities** (`loop`, `drum`, `bass`, `sequence`, `sphere-wave`,
  `effect`, `voice`, `other-sample`): Eight fixed colors shared across the
  Sounds browser (20% row overlay) and Arrangement clips (full category fill
  with dark ink text).
- **Metering & Reduction Lamps** (`meter-ivory`, `meter-green`, `meter-amber`,
  `meter-red`): Drive the Input VU scale face, -14 LUFS target band,
  gain-reduction LEDs/bars, -1 dBTP ceiling line, and latched `OVER` indicator.

### Tertiary

- **Tool & Board Palettes**: Marine cyan hosts Time Stretcher and the FX returns
  bay; Granite hosts Sample Editor; Red hosts Effect Studio; Olive hosts Step
  synth; Indigo hosts the Mastering rack bay; Utility hosts utility and blocking
  dialogs. In Vintage Red, focus rings fall back to warm cream (`modular-cream`)
  because the Red accent reaches only 2.6:1 against its chassis.
- **Polarity & Override Highlights**: `mixer-amber` marks inverted polarity on
  Mixer and Mix view strips.

### Neutral

- **Workspace Surfaces & Ink** (`canvas`, `panel`, `raised`, `hover`, `line`,
  `text`, `muted`, `selection`, `warning`, `drag-label`): Separate timeline
  chrome, browser rows, dividers, and pointer drag badges.
- **Hardware Chassis Neutrals** (`modular-ink`, `modular-cream`, `modular-face`,
  `modular-primary`, `modular-danger`): Define dark recessed bays, value wells,
  pale-metal primary triggers, and rust-red destructive triggers (`Confirm
  discard`), while `mixer-metal` and `mixer-ink` cover legacy ivory Mixer
  strips.

### Bespoke Pedal & FX Motifs

The 13 Mastering pedals and 4 FX units render 16 procedural `MotifStyle` decks
(`GraphicGrid`, `SteppedFrame`, `WoodCheeks`, `DiagonalStripe`, `TubeVents`,
`AluminumCurve`, `RackMeterDeck`, `RadialSunburst`, `AtmosphericGradient`,
`TapeDeckInset`, `PhaseBezel`, `TriBandWells`, `DualSubBays`, `CosmicNebula`,
`DriveRail`, `EmptyBay`). Each pairs a scoped `KnobStyle` (`Default`,
`Anodized`, `DarkPhenolic`, `BrushedChrome`) and preserves >= 5:1 ink contrast
across both lit and bypassed states.

### Color rules

**The Shared Category Rule.** Never assign a different color to the same sound
family between the Sounds browser and the Arrangement, and always pair color
with a visible name or category label.

**The Theme Material Isolation Rule.** Keep Studio flat matte, Aero molded
plastic, and Vintage brushed metal strictly separate; tool bodies must ask for
theme roles or shared painters rather than branching on a theme enum.

**The Five-to-One Enamel Contrast Rule.** Every custom stompbox face and FX deck
must preserve at least 5:1 text contrast across all silk-screen zones in both
active and bypassed states.

## Typography

**Display Font:** Syncopate Bold (700) — reserved strictly for the MIXJAM
branding across every theme.

**Heading Font:** IBM Plex Sans SemiBold (Studio) / Michroma at 86% scale (Aero)
/ Barlow Condensed SemiBold at 120% scale (Vintage).

**Body Font:** Inter Medium (Studio) / Fira Sans Medium (Aero) / Barlow Medium
(Vintage), with Inter Medium as the shared symbol fallback.

Heading faces pin every uppercase title row to a uniform 1.041 em line box via
measured per-face vertical rise offsets (`0.1555 em`, `0.176 em`, `0.2162 em`)
so Aero and Vintage never reflow shared panel geometry.

### Hierarchy

- **Display** (700, 20px base / 11px & 9px subtitle): `MIXJAM` header identity
  paired with muted `TECHNO` sub-mark.
- **Headline** (600, 20px) & **Section** (600, 18px, uppercase): Arrangement/Mix
  header and floating tool window titles.
- **Title** (600, 15px, uppercase) & **Module Tab** (600, 16px): Top-bar project
  name (wrapping to a 9px floor below full width) and lower-panel board titles.
- **Body & Button** (500, 13px, 1.21 line-height): Form labels, actions, tool
  inputs, and track headers.
- **Clip / Small / Metadata** (500, 12px / 11px / 10px): Arrangement clip names
  (dropping to 10px under 28pt height), compact resets, and pedal eyebrows
  (`8.5px`).

### Typography rules

**The Name Remains Rule.** Keep clip sample names visible at every zoom and lane
height (one line below 38pt, up to two lines taller), truncating gracefully with
full names in tooltips.

**The Latin Uppercase Transliteration Rule.** Transliterate Romanian diacritics
(`Ș`, `Ş` -> `S`, `Ț`, `Ţ` -> `T`, `Â`, `Ă` -> `A`, `Î` -> `I`) before uppercase
display rendering while preserving exact spelling in stored names and paths.

## Layout

The native desktop viewport opens at 1440x1000 logical points with a strict
960x720 minimum at 100% display scaling. Structural panels dock in
outer-to-inner priority: 64pt top bar (narrowing 42x42 transport keys to 34x40
below 1240pt), 30pt status bar, 84pt labeled (or 64pt icon-only) left tool rail,
optional lower board panel, 250pt collapsible Sounds panel, 280pt collapsible
Inspector (28pt strips when folded), and the Arrangement canvas (`60pt`
overview, toolbar, `26pt` ruler, `250pt` lane headers).

Lower boards claim preferred minimum heights (`Mixer` 400pt, `Mastering` 346pt,
`FX returns` 320pt, `Step synth` 580pt), clamping down to 370pt (`300pt` in Mix
view to preserve the 70pt Master/returns footer) so track lanes keep at least
150pt of height. Floating windows open to content-fit sizes (`300x160` minimum),
collapse in place to their 44pt header bar, and use solid scrollbars when
overflowing; scrollable panes draw persistent 4pt floating scrollbars over
content without consuming width.

## Elevation & Depth

Studio relies on tonal separation and hairline 1pt borders with flat borderless
popups. Aero shades raised widgets vertically (+6% top highlight to -10% foot
shade), adds a soft white upper gloss (alpha 10/255), and rims controls with
inner highlights over translucent tool bays. Vintage sits brushed olive-sage
plates inside stark rack cabinets with recessed seams, slotted corner screws,
and 3D hardware bevels.

### Shadow Vocabulary

- **Window Shadow**: Soft drop shadow (`[0, 5]`, blur 12, black alpha 85/255)
  beneath every expanded or collapsed floating shell, plus black alpha 85/255
  backdrop under blocking modals.
- **Mastering Pedal Lift** (`mastering-pedal`): Upper-left 135 deg key light
  casting `[5, 8]` blur-16 ambient + `[1, 2]` blur-4 contact shadows under a 4pt
  extruded body, paired with a 6pt lit bevel and 4pt shade walls on Vintage
  cards.
- **Aero Card Drop** (`aero-card`): Quiet `[0, 3]` blur-8 shadow (alpha 100/255)
  lifting molded-plastic stompbox and FX shells above the rack bay.

### Depth rules

**The Code-Only Hardware Rule.** Render every chassis bevel, screw, rotary dial,
meter, and pedal face with bounded procedural geometry in `egui` — no runtime
hardware bitmap tiles outside the brand mark, rail alpha masks, and retiring
Mixer strips — and keep state changes immediate with zero motion tokens.

## Shapes

Corner radii derive centrally from `Roles::corner(base)`: Studio compresses
radii by `0.5x` (max `4pt`), Aero expands them by `1.6x` (max `14pt`, clips
capped at `8pt` so names clear corners), and Vintage keeps nominal radii (`9pt`
on 3D metal pedals, `8pt` on rack plates, `4pt` on Studio flat cards, `10pt` on
Aero plastic cards). Resting controls take 1pt borders; selection and keyboard
focus take 2pt rings. Rail tools mark selection via a 2pt left accent bar and
reserve accent outlines strictly for keyboard focus.

Every Mastering pedal and FX return unit enforces an uninterrupted nameplate
band above its deck rule: decorative motif frames, sunbursts, bezels, stripes,
and phase waves stay bounded inside `HEADER + 4.0 .. FOOTER_TOP - 4.0`
(Mastering) and `y = +72 .. -48` (FX) so strokes never strike through header
titles or corner screws.

## Components

### Buttons

Workspace primary (`action-primary`, Accent fill / dark ink) and secondary
(`action-secondary`, Raised fill) buttons share 30pt heights with 2pt focus
outlines. Hardware shell buttons unify pale-metal primary (`modular-primary` /
`modular-ink`), dark secondary (`modular-face` / `modular-cream`), and rust-red
destructive triggers (`modular-danger`), depressing 1pt when pressed.

### Chips

Sounds browser category chips toggle active filters with a swatch and text label
(`22pt` height). Under the Mastering pedalboard, a `30pt` well strip renders all
13 pedals as signal-order color chips highlighted with each unit's family accent
bar and a cream viewport outline around scrolled cards.

### Cards / Containers

- **Mastering Pedalboard**: `228pt`-tall hardware cards (`184pt` min width,
  `220pt` for 3-knob units) linked by `22pt` directional patch cables between an
  enlarged `136pt` Input VU + Trim unit and `180pt` Output LUFS/true-peak +
  `OVER` unit. Each card holds a `36pt` clean nameplate band (optional `Booster`
  eyebrow at `y = 11`, `13pt` title, `38x20` `Edit` button, glowing active
  lamp), inline `GR` LEDs or `164pt` 3-band strip, full `64pt` dial columns
  (`56x20` wells, `13pt` readout, `11.5pt` label), and `28x24` left/right
  reorder buttons bracketing a `30x22` center bat toggle. Clicking `Edit` or
  double-clicking opens `Mastering deep edit` with response curves and stereo
  vectorscope.
- **FX Returns Rack**: Four `228pt`-tall cards (`198pt+` wide) side-by-side on
  the Marine bay exposing `In`, `S`, `Lim`, `Edit`, effect/preset pickers, up to
  4 full-size macro dials (`56x20` wells when wide, compact at minimum width),
  return meters, and `Clear tail` above `y = +72 .. -48` motif decks, paired
  with the `824x572` `FX Deep Edit` window with two-row breathing bays and
  module-scoped knob caps.
- **Floating Windows & Channel Strip**: One resizable, collapsible shared
  hardware shell wraps Time Stretcher (`Marine`, `Original BPM` & `Target BPM`
  modular bays with live ratio readout), Sample Editor (`Granite`, `Trim Range`
  visual bar & `Pitch & Speed` bay), Effect Studio (`Red`, four accented modular
  bays with `bat_toggle` switches and module-scoped knobs), Help windows,
  blocking confirmations, and `Track N EQ` (`736x500` shared instrument scope
  and dial window).

### Inputs / Fields

Shared hardware dials provide 150pt vertical full-range drag, `Shift` fine
precision, scroll-wheel stepping, double-click default reset, and recessed dark
value wells accepting keyboard typed entry with optional units.

### Navigation

The top bar pairs transport, BPM, and project status; the left rail groups File,
Clips, Mix, Sound, and Library actions above collapsible icon mode; lower-panel
tabs switch Mixer, Mastering, FX returns, and Step synth in place; `Ctrl+K`
opens the searchable 580pt command palette; status bar panel switches persist
workspace visibility.

### Arrangement, Mix View, Mixer & Step Synth

Arrangement lanes support 20/30/40pt row heights, inline Mix view track/returns
controls, a legacy 108pt-wide Mixer board with piecewise dB faders and compact
`Unity`/`Center`/routing switches until retirement, and a `836x512` Step synth
board (`16` steps x `25` visible pitch rows).

## Detailed design pages

These pages hold the per-panel chapters and state matrices.

### Foundations

- [Overview](design/overview.md#overview): the design goal, the three themes,
  the key characteristics, and the code that paints the interface.
- [Themes](design/themes.md): Studio, Aero, and Vintage, and what each changes.
- [Colors](design/colors.md): primary, secondary, and neutral roles, the tool
  palettes, the Mastering and FX motifs, and Mixer tokens.
- [Typography](design/typography.md): faces, sizes, and text roles.
- [Layout](design/layout.md): window regions, sizes, and spacing.
- [Elevation, depth, and shapes](design/elevation-and-shapes.md): layering and
  corner rules.

### Component pages

- [Actions, rail, and Sounds panel](design/components-shell.md): transport,
  library loading, tool rail, Sounds panel.
- [Command list and status bar](design/components-commands-and-status.md):
  command list, status bar.
- [Inspector, overview, and top bar](design/components-score.md): Inspector,
  panel room, song overview, top bar.
- [Score toolbar and Maximise](design/components-score-toolbar.md): score
  toolbar and Maximise.
- [Sounds, clips, and Mix view](design/components-clips.md): clip drawing and
  the Mix view.
- [Mixer, tool windows, and FX returns](design/components-boards.md): the boards
  of the lower panel, and the shared window shell and controls.
- [Sound tools and Album Review](design/components-tools.md): Time Stretcher,
  Effect Studio, Sample Editor, Album Review.
- [Step synth](design/components-step-synth.md): the Step synth board.
- [Options, dialogs, and feedback](design/components-dialogs.md): Options,
  utility windows, blocking dialogs, empty states.

## Do's and Don'ts

### Do

- **Do** give every surface the current theme's roles and material, keeping
  `Syncopate` for the `MIXJAM` product mark in all themes.
- **Do** keep track numbers stable, expose explicit accessible names on every
  widget, and verify changed layouts at `960x720` (`100%` display scaling).
- **Do** bound all pedal and FX motif borders below the header divider
  (`HEADER + 4pt` / `y = 72`) without a redundant top edge stripe.
- **Do** preserve the owner-accepted compact exceptions: Mixer
  `Unity`/`Center`/rails below 24pt, Step synth `14pt` cells on a `16pt` row
  pitch, icon-only collapsible rail mode (names in tooltips), a dual category
  menu + chip row in Sounds, and an omitted web/mobile `Platform` tag for
  Windows desktop.

### Don't

- **Don't** mix materials across themes (no metal bevels or ivory strips in
  Studio/Aero; no plastic gloss in Studio) or resize controls for a single
  theme.
- **Don't** branch on a theme inside tool bodies or interaction logic; add
  semantic roles to the theme registry or extend shared painters instead.
- **Don't** use color as the sole cue for state, selection, or sound category
  identity.
- **Don't** put application rules in the visual layer or cite screenshots as
  evidence for DSP parity, hardware behavior, or screen-reader support.
- **Don't** render decorative frames or corner hardware overlapping pedal
  nameplate text or move buttons.
