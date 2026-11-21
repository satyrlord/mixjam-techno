# Elevation, depth, and shapes

## Elevation & Depth

Flat color layers and thin borders separate the graphite work area. Folded
Metal windows add folded painted-metal edges, dark inset control bays,
recessed values, fine grain, and sparse hairline scratches. Use no decorative
screws or fasteners. The folded lip, texture, and light wear give the hardware
character. Do not add heavy damage, oversized bevels, or ornamental hardware.
Mixer keeps its prepared material and faders.

The Mastering pedals stand off the Indigo board on a soft drop shadow down and
to the right. They have a 6-point bevel and 4-point side walls. The light
falls from the upper left (see
[Mastering board](colors.md#mastering-board)). The FX units use the same
raised enclosures on their Marine board. Floating windows share one window
shadow.

The shared control rendering supplies the hover, keyboard focus, pressed,
selected, and disabled states. Keep state changes immediate and visible.
Color alone does not show a state. The application has no motion tokens.

## Shapes

Clips use the smallest corner radius. Controls and sound rows share the
control radius. Transport has a slightly larger radius. Floating windows use
the window radius. Resting native widgets use one-point borders. Selected
clips use a two-point border in the Text color.

An active rail tool has a two-point accent bar at the left in every theme. On
a bare tool, the bar is at the edge of the target. On a Vintage key face, it
is 2 points inside the border. An accent outline on a rail tool means keyboard
focus only.

Transport keyboard focus uses a two-point inset outline. On the bright
Play/Pause button, the outline is the dark on-accent ink of the theme (Canvas
in Studio). On the secondary controls, it is Accent. Header actions, category
chips, and empty-state actions use the same outline. The surface of an open
menu, context menu, or drop-down list is a Tab stop. While it has keyboard
focus, it shows the Accent outline with the corner radius of the menu, over
its rows. When no step is available, Undo and Redo dim and disable. An engaged
Loop keeps a one-point Accent outline. Window edges, title bars, and the bar
ruler are pointer surfaces and get no Tab stop.

Folded Metal windows share rounded chassis edges and the same control
geometry. The chassis panel corners are 8 points on Vintage painted metal,
6 points on the Studio flat panel, and 14 points on Aero molded plastic.
Native windows and popups use a 10-point base radius, and menus a 6-point base
radius. `Roles::corner` gives each its shape in the theme.

Paint the perimeter and bevel as continuous antialiased native geometry. Fade
the raster material before the outer outline and at strip ends, so that the
joints stay smooth. Prepare knob edges at 8x resolution. Then reduce them one
time, with premultiplied-alpha Lanczos filtering, to the prepared size. Live
text and header symbols stay independent of the material artwork.
