# Mixer, Mastering, and FX returns

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

A mixer row holds the sound names, M, S, L, R, polarity, the fader with its
exact dB value and Unity, pan with Center, the track state, the four sends,
and EQ. Below the lanes, the Master row (with Bypass and Export tracks…) and
the four FX returns stay in view while the lanes scroll. The rows and the
strips have the same controls with the same names. Thus the text below applies
to both.

The Mix view and the Mixer board do not show at the same time. When you
select Mix, the lower panel changes to Mastering. When you select Mixer in the
rail, the score changes back to Arrange.

In a narrow window, not all columns of the Mix view fit. To see the other
columns, drag the bar under the column captions, or turn the wheel over the
rows while you hold Shift. A control that you reach with Tab scrolls into
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
  change the gain or the pan. Amber shows that the polarity is inverted. The
  polarity setting stays the same at silence.

### Outputs, Mute, and Solo

- **L** and **R** switch each output on or off. When both are off, the track
  shows No output.
- **M** mutes the track.
- **S** solos the track. While a track is soloed, only the soloed tracks play.
  A muted track stays silent.

The M, S, L, and R controls in the lane headers and in the Mixer show the same
state. These controls do not change the pan.

### Channel EQ

**EQ**, beside M, opens the channel EQ window of the track. The button comes on
while the EQ is on. The EQ has these bands:

- Low cut and High cut, at 12 dB per octave.
- Low shelf and High shelf.
- Mid peak, from 150 Hz to 10 kHz, with its Q.
- Peak 2, from 40 Hz to 16 kHz, with its Q. Use it for a narrow cut, for
  example one drum line that rings, while Mid peak does a different job.

The window shows the response curve of the EQ. The EQ processes the clips of
the track before the automation and the fader. The EQ of each track starts
off. An EQ that is off keeps its settings and does not change the track.
**Reset** sets all bands flat.

The **EQ** button comes on while one of these is true:

- The EQ, the drive, the compressor, or the ducker is on.
- The width is not 100%.

### Drive

Below the EQ, the window holds the drive of the track. The drive adds
harmonics. Thus a held bass that is almost a pure tone becomes audible on
laptops and phones, which cannot play its fundamental.

To use the drive:

1. Switch on the drive.
2. Set **Amount**: how hard the drive pushes a copy of the track.
3. Set **Character**. At 0%, the drive bends the two halves of the wave
   equally, for odd harmonics, like tape. At 100%, it bends one half, for even
   harmonics, like a tube.
4. Set **Mix**: how much of the change the drive adds.
5. Set **Focus**. Below this frequency, the track stays clean. For a bass, set
   Focus to about twice its lowest note. The fundamental then stays the same,
   and only the harmonics are added.
6. Set **Output** to change the level by hand.

The drive keeps the power of the track. Thus the track does not become louder
when you switch on the drive. The drive works after the EQ and before the
compressor, so the fader does not change how hard it drives. When Mix is 0%
and Output is 0 dB, the drive does not change the track.

### Compressor and width

Between the drive and the ducker, the window holds the compressor of the
track. The compressor makes the level of the track more even after its EQ and
drive: for example, the hits of a drum loop. Switch it on, then set
**Thresh** (the level above which it compresses), **Ratio**, **Attack**,
**Release**, and **Makeup**. The compressor works before the width, the
automation, and the fader of the track. Thus the fader does not change how
hard it compresses.

**Width**, under **Stereo** at the end of the row, sets the stereo width of the
track before its pan:

- 0% changes the track to mono.
- 100% does not change the track.
- 200% doubles the difference between the two sides.

Width keeps the mono sum of the track. Below 100%, the stereo sound comes
nearer to that sum. Above 100%, it moves farther from it. Width works only
while both outputs are on.

### Ducker

The same window holds the ducker of the track. The ducker makes the track
quieter while a different track, its key, plays: for example, a bass that
makes room for the kick.

To use the ducker:

1. Switch on the ducker.
2. Select the key track.
3. Set **Thresh**: the level of the key above which the track ducks. The
   ducker measures the key after its fader and before the master gain. Thus
   the master gain does not change the ducking.
4. Set **Depth**: the maximum ducking.
5. Set **Attack** and **Release**.

The track ducks by the amount that the key goes above the threshold. The
ducker works after the fader of the track and before its sends. A muted key
does not duck the track.

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

The application writes one file for each track that plays, and one for the
return of each FX bus that a track feeds. A track file holds the track alone,
after its EQ, drive, compressor, width, automation, fader, pan, outputs, and
ducker, without its sends. The FX return files hold the sends of all tracks.

The files do not include the master pedalboard. They have the same length as
the mix export. If the summed mix does not clip, the files add up to the mix
with the board bypassed. If one track or FX return goes above the 16-bit
range, the export stops. Make the named track or return quieter, or lower the
master gain, and then export again.

## Mastering

Mastering is the second board of the lower panel. It holds the master
pedalboard:

1. The mix goes into the **Input** unit. Use **Trim** to set its level against
   the VU meter. After Trim, the mix is held to full scale. If the VU meter
   shows that the mix is above full scale, lower Trim.
2. The mix goes through each pedal that is on, from left to right.
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
seventeen genre boards. A preset sets all pedals and their order, and it
switches the Booster pedals off. It keeps your Trim, so a mix that you trimmed
down stays down. It also keeps the keys of the pedals. Undo restores the board
before the preset.

**Bypass all**, beside the selector, skips the full board, Trim included, for
playback and export. Use it to master the exported mix in a different
application. The pedals keep their settings and the board dims. Presets do not
change this switch.

## FX returns

FX returns is a board of the lower panel. It holds the four FX buses that the
Mixer sends feed. The returns go into the mix before the master pedalboard.

To set a bus:

1. Choose Echoform Delay, Aetherform Reverb, or Emberform Drive for the bus.
2. Load a preset.
3. Set its controls.

The bus controls:

- **Sync** sets the delay in note lengths at the song tempo.
- **Input** off lets the tail continue. **Clear tail** stops it immediately.
- An export lets the tails continue after the end of the song.
- The ducker of each effect makes its repeats or tail quieter while its input
  plays. To follow a track instead, for example the kick, set **Key** to the
  track number. A preset keeps the Key.

### Emberform Drive

Emberform Drive is a parallel distortion. The dry sound of the sends stays
clean, and the bus adds its saturation beside it.

- **Type** sets the curve: Tube, Tape, Overdrive, Fuzz, Hard clip, Foldback,
  Rectifier (an octave up), or Bitcrush. For Bitcrush, **Bits** and **Rate**
  set the steps and the sample rate.
- **Drive** pushes the input into the curve, and the output keeps its level.
- **Asym** adds even harmonics.
- **Low cut** and **High cut** set the band that the drive distorts. Thus a
  bass can keep a clean sub.

Twelve presets supply warmth, glue, drum crunch, bass grit, fuzz, clipping, a
telephone band, an octave, folding, two types of crushing, and a ducked pump.
A drive return has no delay, so it stays in phase with the dry sound. Its
safety limiter bends the peaks immediately and does not look ahead.
