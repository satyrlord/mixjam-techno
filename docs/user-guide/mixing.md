# Mixer and Mastering

## Contents

These parts have their own pages:

- [Channel EQ](mixer-processing.md#channel-eq)
- [Drive](mixer-processing.md#drive)
- [Compressor and width](mixer-processing.md#compressor-and-width)
- [Ducker](mixer-processing.md#ducker)
- [FX returns](fx-returns.md#fx-returns)

## Mixer

Use the Mixer for track gain, pan, L/R routing, Mute, Solo, and master volume.
Each strip shows its track number and the names of the sounds on that track.
Master stays in view while you scroll through the tracks.

- To move the fader or the effect knob under the pointer, turn the mouse
  wheel. For fine steps, hold Shift.
- To change the pan, drag it or use the arrow keys. Over a pan control, the
  wheel scrolls the view.

Gain and pan change smoothly while the song plays. This is also true when you
undo or redo them.

### Mixer board and Mix view

The mixer shows in two places:

- **The Mixer board** in the lower panel shows one strip for each track. To
  open it, select **Mixer** in the tool rail.
- **The Mix view** changes each lane header into a mixer row, and the clips
  stay beside it. To open it, select **Mix** in the score toolbar, or **Mix
  view** in the command list (**Ctrl+K**).

A mixer row holds these items:

- The sound names.
- M, S, L, R, and polarity.
- The fader with its exact dB value and Unity.
- Pan with Center.
- The track state, the four sends, and EQ.

Below the lanes, the Master row (with Bypass and Export tracks…) and the four
FX returns stay in view while the lanes scroll. The rows and the strips have
the same controls with the same names. Thus the text below applies to both.

The Mix view and the Mixer board do not show at the same time. When you
select Mix, the lower panel changes to Mastering. When you select Mixer in the
rail, the score changes back to Arrange.

In a narrow window, not all columns of the Mix view fit. To see the other
columns, drag the bar under the column captions. You can also hold Shift and
turn the wheel over the rows. A control that you reach with Tab scrolls into
view. If **Starting module** is Mixer, the score starts in the Mix view.

### Gain, pan, and polarity

- Gain is from -inf dB (silence) to +12 dB. 0 dB is unity. Negative values
  make the signal quieter. The fader gives more travel to the levels that are
  most useful for a mix.
- To type an exact gain, click the gain value. To set 0 dB, select the small
  **Unity** button at the 0 dB mark of the fader.
- Pan shows left, center, or right. To set the pan to the center, select the
  small **Center** button at the middle of the pan control.
- The slashed-circle button inverts the polarity of both channels. It does not
  change the gain or the pan. The button is amber when it inverts the polarity.
  The polarity setting stays the same at silence.

### Outputs, Mute, and Solo

- **L** and **R** switch each output on or off. When both are off, the track
  shows No output.
- **M** mutes the track.
- **S** solos the track. While a track has Solo on, only the tracks with Solo on
  play. A muted track stays silent.

The M, S, L, and R controls in the lane headers and in the Mixer show the same
state. These controls do not change the pan.

### Sends and FX returns

The four small knobs under the fader send the track to FX 1 to FX 4. The range
is from 0 to 100%. The sends come after the fader and the pan. A ring around a
knob shows that it has an automation lane, which sets the send during
playback.

After the tracks, there is one return strip for each bus. It sets the return
level and switches the input and the limiter of the bus. **Edit** opens the
bus on the FX returns board. A ring around a return knob, with the text Return
automated, shows that it has an FX return lane. That lane sets the level
during playback.

### Master bypass and track export

**Bypass** on the Master strip skips the master pedalboard. It does the same
as **Bypass all** on Mastering.

To export each track to its own WAV file, for a mix in a different
application:

1. Select **Export tracks…** in the Mixer toolbar or in the Master row of the
   Mix view.
2. Select a folder.

The application writes one file for each track that plays. It also writes one
file for the return of each FX bus that a track feeds. A track file holds the
track alone, after its EQ, drive, compressor, width, automation, fader, pan,
outputs, and ducker, without its sends. The FX return files hold the sends of
all tracks.

The files do not include the master pedalboard. They have the same length as
the mix export. If the summed mix does not clip, the files sum to the mix
with the board bypassed. If one track or FX return goes above the 16-bit
range, the export stops. Make the named track or return quieter, or lower the
master gain, and then export again.

## Mastering

Select **Mastering** in the lower panel to open the master
pedalboard:

1. The mix goes into the **Input** unit. Use **Trim** to set its level against
   the VU meter. After Trim, the board holds the mix to full scale. If the VU
   meter shows that the mix is above full scale, lower Trim.
2. The mix passes through each pedal that is on, from left to right.
3. The mix goes out through the **Output** unit. During playback, it shows the
   loudness in LUFS, the true peak, and an OVER lamp.

The board holds 13 pedals:

- Three Booster pedals: Booster EQ, Booster compressor, and Booster wide. They
  use the master effects of three original products.
- Ten mastering pedals: Soft clip, Tube, Trim EQ, Bus compressor, Maximizer,
  Lift EQ, Tape, Stereo imaging, Multiband, and Limiter.

All pedals start off. To move a pedal, drag it by its name, or use its arrows.
The Limiter always stays last. To scroll to a pedal, click it in the strip
below the board.

Select **Edit** on a pedal, or double-click its name in the strip, to open
**Mastering deep edit**. Select a pedal in the window's signal-order strip
to get its full set of controls. The window shows the master meters and, for
the saturation pedals, a preview of the transfer curve.

### Key dials

The dynamics pedals (Booster compressor, Bus compressor, Multiband, and
Limiter) each have a **Key** dial. At **Own**, the pedal listens to the mix.
If you set Key to a track number, the pedal listens to that track after its
fader:

- The compressors duck the full mix while the track plays.
- The Multiband keeps each band below its ceiling while the same band of the
  track is loud.
- The Limiter keeps the louder of the mix and the track below the ceiling.

A muted key track is silent.

### Presets and Bypass all

The **Preset** selector loads a factory board: Subtle, Revive, Loud, or one of
sixteen genre boards. A preset sets all pedals and their order, and it
switches the Booster pedals off. It keeps your Trim, so a mix that you trimmed
down stays down. It also keeps the keys of the pedals. Undo restores the board
before the preset.

**Bypass all**, beside the selector, skips the full board, Trim included, for
playback and export. Use it to master the exported mix in a different
application. The pedals keep their settings and the board dims. Presets do not
change this switch.
