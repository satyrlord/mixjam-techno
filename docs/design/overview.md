# Overview

This page is part of [DESIGN.md](../DESIGN.md). In this page, "this document"
means DESIGN.md and the pages that it links.

Design goal: **"A clear live-set score"**

The arrangement is the main work area. Graphite surfaces separate clip names,
track numbers, and playback controls. Lime marks primary actions. Category
colors identify the same sounds in the Sounds panel and the arrangement.

The application has three complete themes that the owner selected: Studio
(the default), Aero, and Vintage. Each theme covers every surface: the
arrangement, Sounds panel, Mixer, boards, tools, menus, tooltips,
confirmations, and feedback. See [Themes](themes.md#themes). The rest
of this document describes the shared layout and behavior, which no theme
changes. Where it names Folded Metal, ivory strips, or enamel finishes, it
describes Vintage, which keeps the prepared artwork. Studio and Aero paint the
same surfaces in code with their own materials.

In Vintage, every in-app window and modal uses Folded Metal: subtly aged
folded painted metal, dark inset control bays, cream text, polished silver
knobs, and pale metal primary buttons. Fine grain, minor scratches, and
restrained edge wear suggest a well-kept synth from the early 2000s. Studio
paints the same shells as flat software panels, and Aero paints them as molded
plastic.

Every theme keeps the tool palette identities, in its own shades:

- Marine cyan for Time Stretcher and the FX returns board.
- Granite gray for Sample Editor.
- Synth red for Effect Studio.
- Olive for Step synth.
- Indigo for the Mastering board and its help.

Utility dialogs use the neutral body of the theme: graphite in Studio, cobalt
in Aero, and neutral metal in Vintage, with an optional orange-yellow accent.
The orange-yellow accent does not replace the tool identities. One shell and
control family covers musical tools and utility tasks. Vintage keeps the warm
graphite workspace and the graphite and ivory Mixer strips. Studio and Aero
paint the workspace and strips in their own materials. Native operating-system
windows and file pickers keep their OS presentation.

RaveFold supplies artistic references only. It does not control product
behavior. If `local-data/research/PRODUCT.md` and
`local-data/research/COMPATIBILITY.md` exist, they control it.

**Key Characteristics:**

- Dark surfaces with off-white text and restrained borders, in the material
  of each theme.
- Syncopate for the product name, uppercase titles in the heading face of the
  theme, and the text face of the theme for all other text.
- A wide arrangement with the transport above, the tool rail and Sounds
  panel at the left, the Inspector at the right, and the boards below.
- Lime primary actions, category-colored clips, and coral recording signals.

The implementation is native Windows Rust with eframe/egui:

- The modules of `src/ui/view/` paint the workspace: the top bar, rail,
  status bar, score, clips, and the boards of the lower panel.
- `src/ui/theme.rs` holds the theme roles.
- `src/ui/mixer/` paints the Mixer.
- `sounds.rs`, `inspector.rs`, `overview.rs`, `mix_view/`, and
  `commands.rs` hold the side panels, the song overview, the Mix view, and the
  command list. `room.rs` decides which panels fit.
- `src/ui/dialog.rs` owns every floating shell and its lifecycle.
- `src/ui/controls/` owns the rendering and interaction of reusable controls.
- `src/ui/stretch.rs`, `src/ui/effects.rs`, and `src/ui/sample_editor.rs`
  hold the tool window bodies. `src/ui/step_synth.rs` holds the Step synth
  board. `src/ui/tools.rs` holds the task adapters.
- `src/application.rs` holds the application state and behavior.

Sizes in this document use portable `px` labels for egui logical points. The
front matter records native component metadata. It does not define browser
components or application behavior.
