# Mixer channel processing

## Channel EQ

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

## Drive

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

## Compressor and width

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

## Ducker

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
