# Agent control: Step synth, arrangement, and Track EQ

## Step synth board

The Step synth board is in `state.windows` as `Step synth` while the board
shows. It is a board in the lower panel and not a tool window. Use
`window="Step synth"` to scope `ui`, `click`, and `set` to it.

`state.step_synth` has these fields:

- `shown` and `effects_shown`: the board shows, and its Effects pane shows.
- `view_bar`: the first bar in view, counted from 1.
- `octave`: the octave in view, -3 to 3. The 25 pitch rows go from
  `12 * octave - 12` to `12 * octave + 12` semitones.
- `selection`: the first and last selected step, or `null`.
- `mixed_lanes`: the value lanes (`Accent`, `Slide`, `Velocity`) in which the
  selected notes differ. The board draws a dot at the left of such a lane. An
  edit of such a lane shifts the selected notes together. With `ui all=true`,
  the control of that lane (`Accent lane`) has a description that starts with
  `Mixed`.
- `root_name`, `notes`, and `pattern`: the scale root, each note, and the whole
  pattern. Each note has its `pitch` in semitones from the source pitch, and its
  `note` name. The names go from `A` (the source pitch) up to `G#`.

Each pitch cell is named from its step and its pitch, such as
`Step 16, -12 semitones`. The description of the cell has the note name and
the scale: `Note A, in scale.` or `Note C#, not in scale.` Read it with
`ui all=true contains="Step 16"`.

## Arrangement edits

The arrangement lanes are one painted surface. These requests edit them
without positions: `place`, `move_clips`, `delete_clips`, `select`, `seek`,
`set_loop`, `set_sections`, `automate`, and `clip`. These edits use the same
history as manual edits.

`set_sections` replaces the named sections of the song. Each section has its
first `bar` (or `tick`) and its `name`. `state` lists them under
`song.sections`.

`clip` sets the gain, trim, length, fades, pitch, and `reverse=true|false` of
one clip. `state` gives the `reverse` of each clip.

### Track automation

`automate track=N lane=LANE points=[[BAR,VALUE],...]` replaces one automation
lane. `points=[]` clears it. The lanes are these:

- `volume`.
- `send1` to `send4`.
- `filter`.
- `pan`: -1 hard left to 1 hard right, 0 center.
- `repeat`: 0 off, or 4, 8, 16, 32, or 64 for 1/4 to 1/64 repeats.
- `repeatgate`: 5 to 100, the percent of each repeat that sounds.
- `pitch`: -4800 to 2400 cents of playback speed.
- `compression`: 0 to 100, the percent of the track's compressor in the mix.

The repeat lane steps. It is off before its first point, and holds each value
until the next point. Thus `points=[[5,8],[5.25,16],[5.375,32],[5.5,0]]` rolls
from 1/8 through 1/32 on the slice that starts at bar 5.

The repeat gate also steps, and is 100 before its first point. Each repeat
takes the value at its start. Thus
`automate track=2 lane=repeatgate points=[[5,25],[5.5,100]]` makes the repeats
from bar 5 short plucks.

The pitch lane is varispeed, as on a tape. Pitch and speed move together, and
add to the pitch of each clip. The lane is 0 before its first point and moves in
straight lines. Thus
`automate track=3 lane=pitch points=[[8.5,0],[9,-4800],[9.0625,0]]` is a tape
stop over the second half of bar 8.

The compression lane holds its first and last values outside its points and
moves in straight lines between points. An empty lane gives full compression.
Its values apply while the track compressor is on.

`state` lists the lanes of each track that have points under its `automation`.
The gate shows as `repeatGate`. A send or pan lane there replaces the knob in
`sends` or `pan`.

### Bus automation

`automate bus=N lane=return points=[[BAR,VALUE],...]` replaces the return lane
of FX bus N. The values go from 0 to 2: 1 is the full return, and 2 is
approximately +6 dB. While the lane has points, it replaces the Return knob of
the bus. `points=[]` clears it. `state` lists the four buses under
`song.fx_buses`, each with `bus`, `effect`, `powered`, `return_level`, and
`return_automation`.

## Track EQ window

The channel EQ, drive, compressor, width, and ducker of a track are controls in
its EQ window. To open it, click `Track N EQ` in the Mixer or the Mix view, or
`Inspector track EQ` for the selected track. Then `set` the control by name:

1. `set target="Track 1 drive power" value=true`.
2. `set target="Track 1 drive amount" value=18`.
3. Set `drive character`, `drive mix`, `drive focus` (Hz), and `drive output`
   (dB) in the same way.

`state` lists the `eq_on`, `compressor_on`, and `width` of each track, and its
`drive` with its switch and settings.
