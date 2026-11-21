---
name: MixJam Techno
description: A modern native studio for arranging, mixing, and shaping classic sounds.
colors:
  # Workspace values are Studio's, the default theme. The mixer-, folded-,
  # and mastering- values are Vintage's. src/ui/theme.rs holds all three themes.
  canvas: "#15171a"
  panel: "#1c1f23"
  text: "#eceff1"
  accent: "#bee860"
  coral: "#ff8282"
  muted: "#a4aeb6"
  line: "#3c4249"
  raised: "#282c31"
  hover: "#34393f"
  selection: "#3e5228"
  warning: "#e8c585"
  loop: "#d4c182"
  drum: "#d99d78"
  bass: "#a2c485"
  sequence: "#89b0d2"
  sphere-wave: "#7fbeb9"
  effect: "#d99192"
  voice: "#b99ecb"
  other-sample: "#bbb39f"
  drag-label: "#f5dc92"
  mixer-ink: "#1c1d1a"
  mixer-cream: "#f9f2dc"
  mixer-metal: "#c9c2af"
  mixer-edge: "#6c6a61"
  mixer-face: "#1d2122"
  mixer-amber: "#f5c156"
  folded-utility: "#3c4140"
  folded-marine: "#20535b"
  folded-granite: "#34393c"
  folded-red: "#9a2d27"
  folded-olive: "#3c4428"
  folded-utility-accent: "#e79331"
  folded-marine-accent: "#55aeb7"
  folded-granite-accent: "#adbabd"
  folded-red-accent: "#eb7468"
  folded-olive-accent: "#9db25e"
  folded-indigo: "#2e3660"
  folded-indigo-accent: "#96a6ec"
  folded-ink: "#121412"
  folded-cream: "#f9f2dc"
  folded-help: "#beb9a6"
  folded-face: "#232624"
  folded-primary: "#d4cdb6"
  folded-disabled-face: "#3d403c"
  folded-disabled-text: "#c9cbc0"
  folded-danger: "#833627"
  mastering-ivory: "#f0e4be"
  mastering-green: "#6ec470"
  mastering-amber: "#f6b240"
  mastering-red: "#e84838"
typography:
  # Studio's faces; the Themes section gives Aero's and Vintage's.
  display:
    fontFamily: "Syncopate"
    textTransform: "uppercase"
    fontSize: "20px"
    fontWeight: 700
  headline:
    fontFamily: "IBM Plex Sans"
    textTransform: "uppercase"
    fontSize: "20px"
    fontWeight: 600
  section:
    fontFamily: "IBM Plex Sans"
    textTransform: "uppercase"
    fontSize: "18px"
    fontWeight: 600
  title:
    fontFamily: "IBM Plex Sans"
    textTransform: "uppercase"
    fontSize: "15px"
    fontWeight: 600
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
  bottom-module:
    fontFamily: "IBM Plex Sans"
    textTransform: "uppercase"
    fontSize: "16px"
    fontWeight: 600
  folded-title:
    fontFamily: "IBM Plex Sans"
    textTransform: "uppercase"
    fontSize: "18px"
    fontWeight: 600
rounded:
  # Studio's radii; theme.rs Roles::corner gives Aero's and Vintage's.
  clip: "2px"
  control: "2.5px"
  transport: "3px"
  window: "4px"
  mixer-control: "2px"
  mixer-strip: "1.5px"
spacing:
  item-horizontal: "8px"
  item-vertical: "6px"
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
  category:
    backgroundColor: "{colors.panel}"
    textColor: "{colors.muted}"
    typography: "{typography.button}"
    rounded: "{rounded.control}"
    height: "22px"
  category-selected:
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
    backgroundColor: "{colors.folded-face}"
    textColor: "{colors.folded-cream}"
    rounded: "{rounded.control}"
    width: "48px"
    height: "30px"
  bottom-module:
    typography: "{typography.bottom-module}"
    height: "28px"
  mixer-strip:
    backgroundColor: "{colors.mixer-metal}"
    textColor: "{colors.mixer-ink}"
    rounded: "{rounded.mixer-strip}"
    width: "108px"
  mixer-fader:
    width: "24px"
    height: "32px"
  mixer-value:
    backgroundColor: "{colors.mixer-face}"
    textColor: "{colors.mixer-cream}"
    rounded: "{rounded.mixer-strip}"
    width: "92px"
    height: "22px"
  mixer-unity:
    backgroundColor: "{colors.mixer-face}"
    textColor: "{colors.mixer-cream}"
    rounded: "{rounded.mixer-control}"
    width: "44px"
    height: "16px"
  mixer-center:
    backgroundColor: "{colors.mixer-face}"
    textColor: "{colors.mixer-cream}"
    rounded: "{rounded.mixer-control}"
    width: "40px"
    height: "14px"
  mixer-polarity:
    backgroundColor: "{colors.mixer-face}"
    textColor: "{colors.mixer-cream}"
    rounded: "{rounded.mixer-control}"
    width: "26px"
    height: "24px"
  mixer-route:
    backgroundColor: "{colors.mixer-face}"
    textColor: "{colors.accent}"
    rounded: "{rounded.mixer-control}"
    width: "28px"
    height: "24px"
  folded-frame:
    backgroundColor: "{colors.folded-utility}"
    textColor: "{colors.folded-cream}"
    typography: "{typography.folded-title}"
  folded-action:
    backgroundColor: "{colors.folded-face}"
    textColor: "{colors.folded-cream}"
    typography: "{typography.button}"
  folded-primary:
    backgroundColor: "{colors.folded-primary}"
    textColor: "{colors.folded-ink}"
    typography: "{typography.button}"
  folded-danger:
    backgroundColor: "{colors.folded-danger}"
    textColor: "{colors.folded-cream}"
    typography: "{typography.button}"

---

# Design System: MixJam Techno

## Foundations

- [Overview](design/overview.md#overview): the design goal, the three themes,
  the key characteristics, and the code that paints the interface.
- [Themes](design/themes.md): Studio, Aero, and Vintage, and what each changes.
- [Colors](design/colors.md): primary, secondary, and neutral roles, Folded
  Metal palettes, the Mastering board, and Mixer tokens.
- [Typography](design/typography.md): faces, sizes, and text roles.
- [Layout](design/layout.md): window regions, sizes, and spacing.
- [Elevation, depth, and shapes](design/elevation-and-shapes.md): layering and
  corner rules.

## Components

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

## Design rules

### Use

- Give every surface the roles and material of the current theme. Keep
  Syncopate for the product name in every theme.
- Use the same category and sample names across views.
- Keep track numbers stable and give actions accessible names.
- After a change, check the native layouts at the supported minimum size.

### Accepted exceptions

The owner confirmed these after an audit. Do not report them as defects.

- Mixer Unity, Center, pan rail, and gain value stay below 24 points high. The
  compact strip is intentional.
- Step synth note cells stay 14 points high on a 16-point row pitch, so that
  most of the 25 pitch rows show beside the lanes. Arrow keys reach every cell.
- The tool rail shows the name of each tool by default, and the Tools switch
  in the status bar shrinks it to icons. Tooltips name each action, its
  window, and its shortcut.
- The Sounds panel keeps both the category menu and the category row. The menu
  lists the categories of the library, including Uncategorized.
- The product notes leave Platform unclassified. No value of the design skill
  describes a Windows desktop application.

### Avoid

- Do not keep the material of one theme inside another theme: no metal
  chassis or ivory strips in Studio or Aero, and no plastic gloss in Studio.
  Do not add repeated waveform traces.
- Do not branch on a theme in tool bodies or interaction code. Add a role or
  extend a shared painter.
- Do not move or resize a control for one theme.
- Do not use color as the only way to identify a sound or action.
- Do not put application rules in the visual layer.
- Do not use a screenshot as evidence for audio parity, device behavior, or
  assistive technology support.
