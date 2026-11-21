# Control ranges

The application and the tools reject a setting outside the range of its
control. Obey these rules in each fix, proposal, and brief:

- Give each value inside its range. A value is in the unit of its control. It
  is not a percentage of the range, unless the unit is percent.
- Quote a current setting with its control name and its value from the song
  file. Example: "drive amount 18, character 25, mix 50".
- If a control is at its limit, say so, and propose a different control.
- A lane has one EQ, one drive, one compressor, and one ducker. The EQ has a
  low cut, a low shelf, two peaks, a high shelf, and a high cut. Do not propose
  more bands or a second unit.

The song format documents own the range of each control. Read the section for
the control before you give a value:

| Controls | Section that owns the ranges |
| --- | --- |
| Lane fader, pan, and sends. Master gain. The Volume, Send, Filter, Pan, Repeat, Repeat gate, Pitch, and Compression lanes | [Song](../../../docs/song-format/song-and-lanes.md#song), [Lanes](../../../docs/song-format/song-and-lanes.md#lanes), and [`automation`](../../../docs/song-format/song-and-lanes.md#automation) |
| Lane EQ, drive, compressor, width, and ducker | [Lane processing keys](../../../docs/song-format/lane-processing.md#lane-processing-keys) |
| Clip gain, clip pitch, and clip fades | [Placements and samples](../../../docs/song-format/master-fx-and-placements.md#placements-and-samples) |
| Master pedals and trim | [Master pedalboard](../../../docs/song-format/master-fx-and-placements.md#master-pedalboard) |
| FX bus return level, return lane, and effect modules | [FX buses](../../../docs/song-format/master-fx-and-placements.md#fx-buses) |
