# Elevation, depth, and shapes

## Elevation & Depth

Flat color layers and thin borders separate the Studio work area. Vintage adds
olive-sage anodized rack panels with dark recessed seams, machined top bevels,
dark inset bays, slotted corner rack screws on chassis frames, and dark-skirted
silver-cap rotary dials. Avoid heavy surface damage or oversized bevels. The
Vintage Mixer uses prepared strip and fader artwork.

The Mastering pedals stand off the Indigo rack on a soft drop shadow down and
to the right. On Vintage they add a 6-point bevel, 4-point side walls, brushed
metal grain, sheen, and subtle scratches. The FX units share the same raised
motif enclosures on the Marine bay. Floating windows share one window shadow.

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

Chassis windows share rounded edges and the same control geometry. Panel
corners are 8 points on Vintage anodized metal, 6 points on the Studio flat
panel, and 14 points on Aero molded plastic. Native windows and popups use a
10-point base radius, and menus a 6-point base radius. `Roles::corner` gives
each its shape in the theme.

All panels, bevels, and rotary dials render in bounded native geometry without
chassis bitmap tiles, keeping edges smooth at any resize size. Live text and
header symbols scale cleanly inside their bounds.
