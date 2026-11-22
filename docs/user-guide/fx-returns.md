# FX returns

FX returns is a board of the lower panel. It holds the four FX buses that the
Mixer sends feed. The returns go into the mix before the master pedalboard.

To set a bus:

1. Choose Echoform Delay, Aetherform Reverb, or Emberform Drive for the bus.
2. Load a preset.
3. Set the main controls on the card. Select **Edit** to open the other
   controls, the scope, and the return meters.

The bus controls:

- **Sync** sets the delay in note lengths at the song tempo.
- **Power** off stops new input and lets the tail continue. **Clear tail**
  fades it out over 12 ms.
- **Solo** monitors only the selected returns during playback. Export uses
  the full mix.
- An export lets the tails continue after the end of the song.
- The ducker of each effect makes its repeats or tail quieter while its input
  plays. To follow a track instead, for example the kick, set **Key** to the
  track number. **Threshold** sets the level where ducking begins. A preset
  keeps the Key.

## Echoform Delay and Aetherform Reverb

- In Echoform Delay, **Link L/R** makes the left control set both channels in
  Free and Sync modes. **Smear** spreads the repeats, and **Resonance** shapes
  the repeat filters.
- In Aetherform Reverb, **Bass** and **Treble** change how long the
  low and high frequencies ring relative to the main decay time.

## Emberform Drive

Emberform Drive is a parallel distortion. The dry sound of the sends stays
clean, and the bus adds its saturation beside it.

- **Type** sets the curve: Tube, Tape, Overdrive, Fuzz, Hard clip, Foldback,
  Rectifier (an octave up), or Bitcrush. For Bitcrush, **Bits** and **Rate**
  set the steps and the sample rate.
- **Drive** pushes the input into the curve. **Level match** compensates for
  the change in level.
- **Asym** adds even harmonics.
- **Low cut** and **High cut** set the band that the drive distorts. Thus a
  bass can keep a clean sub.

Twelve presets supply these sounds: warmth, glue, drum crunch, bass grit, fuzz,
clipping, a telephone band, an octave, and a wavefolder. They also supply two
types of bitcrush and a ducked pump. During playback, the drive return uses a
soft ceiling without lookahead. Export uses a true-peak safety limiter and
compensates for the processing delay to align the return with the dry sound.
