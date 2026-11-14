# Elevation, depth, and shapes

## Elevation & Depth

Flat color layers and thin borders separate the graphite work area. Folded Metal
windows add folded painted-metal edges, dark inset control bays, recessed values, fine
grain, and sparse hairline scratches. Use no decorative screws or fasteners.
The folded lip, texture, and light wear provide the hardware character. Do not
add heavy damage, oversized bevels, or ornamental hardware. Mixer retains its existing prepared material and faders.

Mastering's pedals stand off the Indigo board on a soft drop shadow cast down
and right, with a 6-point bevel and 4-point side walls, all lit from the upper
left (see Colors). The Effects units use the same raised enclosures on their
Marine board. Floating windows share one window shadow.

Shared control rendering supplies hover, keyboard focus, pressed, selected, and
disabled states. Keep state changes immediate and visible; color alone does not
communicate state. The application has no motion tokens.

## Shapes

Clips use the smallest corner radius. Controls and sound rows share the control
radius; transport has a slightly larger radius. Floating windows use the window
radius. Resting native widgets use one-point borders. Selected clips use a
two-point text-colored border; active rail actions use a one-point accent outline.
Transport keyboard focus uses a two-point inset outline: Canvas on the bright
Play/Pause button and Accent on secondary controls. Header actions, category
buttons, and empty-state actions use the same outline. Undo and Redo dim and
disable when no step is available. An engaged Loop keeps a
one-point Accent outline. Window edges, title bars, and the bar ruler
are pointer surfaces and take no Tab stop.
Folded Metal windows share rounded chassis edges and the same control geometry.
The painted chassis uses 8-point corners. Native windows and popups use a
10-point base radius and menus a 6-point one, each in the theme's shape
language (`Roles::corner`).
Paint the perimeter and bevel as continuous antialiased native geometry. Fade
raster material before the outer outline and at strip ends so joints stay smooth.
Prepare knob edges at 8x resolution, then reduce once with premultiplied-alpha
Lanczos filtering for each 100%, 150%, and 200% variant.
Live text and header symbols stay independent of the material artwork.
