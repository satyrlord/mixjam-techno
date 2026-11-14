# Mixer, Mastering, and FX returns

## Mixer

Use Mixer for track gain, pan, L/R routing, Mute, Solo, and master volume.
Master stays visible while you scroll through tracks. Each strip shows its
track number and the sound names on that track. The mouse wheel moves the
fader or effect knob under the pointer (Shift for fine steps). Over a pan
control, the wheel scrolls the view. Drag or use the arrow keys to change pan.
Gain and pan change smoothly while the song plays, including when you undo or
redo them.

The mixer has two places. The Mixer tab in the lower panel shows a strip per
track. Mix, in the score toolbar, or the Mixer button in the tool rail, turns
each lane's header into a mixer row instead, while the clips stay beside it:
the sound names, M, S, L, R, polarity, the fader with its exact dB value and
Unity, pan with Center, the track's state, the four sends, and EQ. Below the
lanes, the Master row (with Bypass and Export tracks…) and the four FX returns
stay in view while the lanes scroll. The rows and the strips are the same
controls with the same names, so everything below applies to both. The Mix
view and the Mixer tab never show together: choosing Mix moves the lower panel
to Mastering, and choosing the Mixer tab returns the score to Arrange. In a
narrow window, drag the bar under the column captions, or use Shift and the
wheel over the rows, to see the other columns; a control reached with Tab
scrolls into view. When Starting module is Mixer, the score opens in the Mix
view.

- Gain runs from -inf dB (silence) to +12 dB; 0 dB is unity. Negative dB
  attenuates the signal. The rail gives more space to useful mixing levels.
  Click the gain value for exact dB entry, or select the small Unity button
  at the rail's 0 dB mark. Pan shows left, center, or right; its small Center
  button resets pan at the rail midpoint. The slashed-circle button reverses
  both channels' polarity without changing gain or pan. Amber means inverted;
  the polarity setting remains independent even at silence.
- L and R enable each output; both off shows No output. M mutes the track.
  S solos it: while any track is soloed, only soloed tracks play, and a muted
  track stays silent. The lane and mixer M/S/L/R controls mirror each other.
  Changing these controls does not change pan.
- EQ, beside M, opens the track's channel EQ window and lights while the EQ is
  on. The EQ has a low cut and a high cut at 12 dB per octave, a low shelf,
  two peaks with their Q, and a high shelf, and draws its response curve. Mid
  peak runs from 150 Hz to 10 kHz; Peak 2, from 40 Hz to 16 kHz, takes a
  narrow cut, such as one ringing drum line, while Mid peak does another
  job. It acts on
  the track's clips before its automation and fader. Each track's EQ starts
  off; an EQ that is off keeps its settings and leaves the track as it was.
  Reset flattens every band.
- The same window holds the track's ducker, which lowers the track while
  another track, its key, sounds: a bass that makes room for the kick, for
  example. Switch it on, pick the key track, and set Thresh (the key's level
  above which the track ducks, after the key's fader and before the master
  gain, so the master gain does not change the ducking), Depth (the most it
  ducks), Attack, and
  Release. The track ducks by as much as the key rises over the threshold,
  after its fader and before its sends. A muted key does not duck. The EQ
  button lights while the EQ, the drive, the compressor, or the ducker is on,
  or while the width is not 100%.
- Under the EQ, the window holds the track's drive, which adds harmonics: a
  held bass that is close to a pure tone can then be heard on laptops and
  phones, whose speakers cannot play its fundamental. Switch it on and set
  Amount (how hard a copy of the track is driven), Character (0% bends both
  halves of the wave alike, for odd harmonics, like tape; 100% bends one half,
  for even harmonics, like a tube), Mix (how much of the change is added),
  Focus (below this frequency the track stays clean), and Output. For a bass,
  set Focus to about twice its lowest note: the fundamental stays as it is and
  only the harmonics come in. The drive keeps the track's power, so switching
  it on does not make the track louder; Output moves the level by hand. It
  works after the EQ and before the compressor, so the fader does not change
  how hard it drives. Mix at 0% leaves the track unchanged when Output is 0 dB.
- Between the drive and the ducker, the window holds the track's compressor,
  which evens out the track's level after its EQ and drive, such as the hits of a drum
  loop. Switch it on and set Thresh (the level above which it compresses),
  Ratio, Attack, Release, and Makeup. It works before the track's width,
  automation, and fader, so moving the fader does not change how hard it
  compresses. Width, under Stereo at the end of the row, sets the track's
  stereo width before its pan: 0% folds the track to mono, 100% leaves it as
  it is, and 200% doubles the difference between its sides. Width keeps the
  track's mono sum: below 100% the track's stereo sound comes closer to that
  sum, and above 100% it moves further from it. Width works while both
  outputs are on.
- Bypass on the Master strip skips the master pedalboard, as Mastering's
  Bypass all does.
- Export tracks… in the Mixer's toolbar or the Mix view's Master row asks
  for a folder and writes each
  track that plays, and the return of each FX bus that it feeds, to its own
  WAV file, for mixing in another application. A track's file is the track
  alone after its EQ, drive, compressor, width, automation, fader, pan, outputs, and ducker,
  without its sends;
  the FX returns hold every track's sends. The files leave the master
  pedalboard out and are as long as the mix export. They sum to the mix with
  the board bypassed when the summed mix does not clip. Export stops if an
  individual track or FX return exceeds the 16-bit range; lower the named
  track or return level, or the master gain, and export again.
- The four small knobs under the fader send the track to FX 1 to FX 4, from
  0 to 100%, after its fader and pan. A ringed knob has an automation lane,
  which sets the send while the song plays. After the tracks, a return strip for
  each bus sets its return level and switches its input and limiter; Edit
  opens the bus on the FX returns board. A ringed return knob, marked Return
  automated, has an FX return lane, which sets the level while the song plays.

## Mastering

Use Mastering, the third module of the lower panel, for the master
pedalboard. The mix enters the Input unit, where Trim sets its level against
the VU meter; after the Trim the mix is held to full scale, so lower the Trim
when the VU meter shows a mix over full scale. It then passes each pedal that
is on from left to right, and leaves
through the Output unit, which shows loudness in LUFS, the true peak, and an
OVER lamp while the song plays.

The board holds the three Booster pedals (Booster EQ, Booster compressor, and
Booster wide), which use the master effects of three original products,
and ten mastering pedals: Soft clip, Tube, Trim EQ, Bus compressor, Maximizer, Lift
EQ, Tape, Stereo imaging, Multiband, and Limiter. Drag a pedal by its name or
use its arrows to move it; the Limiter always stays last. Click a pedal in the
strip below the board to scroll to it. Every pedal starts off.

The dynamics pedals (Booster compressor, Bus compressor, Multiband, and
Limiter) each have a Key dial. At Own the pedal listens to the mix. Set it to
a track's number and the pedal listens to that track as it sounds after its
fader: the compressors duck the whole mix while the track sounds, the
Multiband holds each band under its ceiling while the track's same band is
loud, and the Limiter holds the louder of the mix and the track under the
ceiling. A muted key track is silent.

The Preset selector loads a factory board: Subtle, Revive, or Loud, or one of
seventeen genre boards. A preset sets every pedal and the order and turns the
Booster pedals off; it keeps your Trim, so a mix you trimmed down stays down,
and the pedals' keys.
Undo restores the board before it. Bypass all, beside the selector, skips the
whole board, Trim included, for playback and export, so you can master the
exported mix elsewhere. The pedals keep their settings, the board dims, and
presets leave the switch alone.

## FX returns

Use FX returns, a board of the lower panel, for the four FX buses
that the Mixer's sends feed. Choose Echoform Delay or Aetherform Reverb for
a bus, load a preset, and set its controls; the returns join the mix before
the master pedalboard. Sync sets the delay in note lengths at the song's
tempo. Input off lets the tail ring out; Clear tail cuts it at once. Export
lets the tails ring past the song's end. Each effect's ducker lowers its
repeats or tail while its input plays; set its Key to a track's number and it
follows that track instead, such as the kick. A preset keeps the Key.
