# Components: sounds, clips, and Mix view

## Sounds and clips

Sound rows show the name above the category label and bar count. A row too
narrow for both keeps the bar count. A 20% overlay
of the category color tints the whole row; there is no category dot.
Hover or selection uses Hover; selection or keyboard focus adds a one-point
category-colored border. The category and bar-count line uses Text at 90%.
Unselected category chips use Panel with Muted text and a one-point Hover
border; the selected one uses Hover, Text, and a one-point Muted border.
One tooltip groups the full name, product, category, code, sample IDs, source
tempo, nominal bars, actual audio duration, frames, rate, channels, and WAV path
when available. Nominal bars and audio duration remain separate facts.
Arrangement clips use flat category fills and names, with the clip's waveform
under the name. The waveform uses the category color darkened to 55% and fills
the band between the name and the clip's bottom edge. In a 20-point lane that
band is too short, so the waveform fills the clip in a lighter tone (82%) and
the name stays readable over it. The waveform shows the part of the sound the
clip plays: it starts at the trim and ends where the sound ends, which can be
before the clip's end. A reversed clip mirrors it, so any silence comes first.
Fades shape it to silence at their ends. Clips never repeat their sound, so the
waveform does not repeat; Pitch-lane varispeed is not drawn. A reversed clip's
`rev` mark sits on a small plate of the clip's fill. A clip whose sound is
missing has no waveform. Only clips on screen paint a waveform, and each clip
keeps its shape until its sound, trim, length, fades, Reverse, tempo, or zoom
changes. A clip of a Step synth sound saved with its pattern draws notes in
place of the waveform, in the same band and tones: one bar per note, 2 to 4
points thick by the lane's height, its pitch mapped over at least an octave,
held to the next note or for one sixteenth with the 1/16 cut, and mirrored by
Reverse. WAV clips never draw notes.
A selected clip can show a gain line; it is a control indicator.
A clip whose source is missing uses Raised with a one-point Coral outline and
a Text-colored name that begins with Missing. While a sound or clip is dragged
over the lanes, clip ghosts at 60% opacity show the destination bar and track.
A drop puts every clip on a row or does nothing. A group that would put a clip
on a track without a row, or outside the timeline, shows no ghosts; a Drag
Label names that track or reads Outside the timeline, and the drop is canceled
with a status message. Off the lanes, a dragged sound shows a 150 by 30-point
Drag Label with its name in Canvas text. A palette sound armed for click
placement shows a tinted outline where it will land. Choosing a clip or pressing Esc
ends click placement, so a click on an empty lane then only clears the
selection. Enter on a focused sound row places
it on the recording track at the playhead. Lane M/S/L/R letters gain a short
bar when their switch is on. The lanes are one Tab stop with an Accent outline:
arrow keys choose a clip, Alt+arrows move the selection by a bar or a track,
Ctrl+Alt+arrows by a sixteenth, Enter previews, Delete removes, and Shift+F10
opens the selected clip's menu under the clip. Delete and Shift+F10 also work
when no control has focus. Ctrl+A selects every clip and Ctrl+D copies
the selection to the bars that follow. A right-click on a clip offers Preview,
Duplicate, Delete, and, for a missing sound, Resolve manually.

A clip is edited on the clip itself, as in other DAWs. Its side edges trim it:
the right edge sets its length, and the left edge moves its start and trim
together, so the sound stays where it plays. Edges snap to bars, or to
sixteenths with Ctrl. Small dark squares on the top edge set the fade in and
fade out; they snap to sixteenths, or to ticks with Ctrl. A small handle on
the gain line, in the middle of the clip's visible part, sets the gain: each
4 points up or down is 1 dB, or 0.1 dB with Ctrl, from +6 dB to silence. A
label at the pointer shows the value, and the release makes one undo step.
Where the gain handle covers another handle, the visible gain handle receives
the input. The rest of the clip still moves it; Shift, Alt, or the right button
copies it. The right-click menu also holds the exact values: gain in dB like
the Mixer, trim start in
frames, length, fades with their curves, and pitch in semitones and cents.
The arrangement has no clip strip under the lanes. Clips with available audio
show their name without a hover tooltip. Missing clips keep their metadata and
resolution guidance in a tooltip.

## Mix view

The Mix view is the arrangement with each used lane's header widened into a
mixer row; the clips keep the rest of the lane, and empty lanes keep their drop
target. A row keeps the lane's height. From the left: the track number and
record arm, the sound names (12 points, 11 in 20-point lanes, elided, with the
full list in the hint), M, S, L, and R, polarity, the level fader with its dB
value and Unity, pan with Center, the state text (Muted, No output, Solo
elsewhere, Pan automated, or the pan position) in 11 points, the four send
knobs, and EQ. The header column is the score's width less 160 points, from
250 to 876 points; the rows' content is 876 points wide. The ruler strip's
header part shows the column captions (Track, Outputs, Level, Pan, Sends, EQ)
in 10-point uppercase Muted text. When the column is narrower than its rows,
everything right of the 34-point number and arm scrolls sideways, and a
3-point bar with a Muted thumb at the captions' foot shows and drags the
offset.

Under the lanes, a 70-point block stays in view while the lanes scroll: the
Master row (Master over "Stereo output", Bypass, the level, and Export
tracks…) and the returns row, with four 210-point groups of the bus name and
its effect, the return state ("Return 88%" or "Return automated"), the return
level knob, Input, Limiter, and Edit. While the FX returns board shows, each
group keeps its name, state, and Edit and reads "On the board below" in place of
the knob and switches, so no control has two names on screen. The rows carry the Mixer's material:
Vintage tiles the ivory strip artwork for the track and Master rows and the
graphite return artwork for the returns row; Studio and Aero fill the strip
and return-strip surfaces. Row text uses the strip ink, and each row has a
1-point strip-edge outline. The track number keeps its arrangement position
without its panel backing.
