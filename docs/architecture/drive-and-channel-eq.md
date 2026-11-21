# Drive and channel EQ

## Drive

The drive comes after the EQ and before the compressor. Thus the EQ chooses
what is driven: for example, a low cut that keeps rumble out of the curve, or
a boost that drives one band harder. The compressor after the drive makes the
driven sound more even. It hears the harmonics that the drive added, so its
detector follows the track as it sounds. Like the compressor, the drive
operates before the fader, so the fader does not change how hard it drives.
It also operates before the automation and the sends, which therefore carry
its harmonics.

The drive (`drive::Run`) does these steps:

1. It bends a copy of the track through its curve at four times the mix rate,
   with antiderivative anti-aliasing on each sample.
2. It matches the power of the copy to the power of the track.
3. It high-passes the difference between the copy and the track at its focus.
4. It adds the share of that difference that Mix sets to the track.
5. It holds the result at the power of the track, before the output trim.

The driven and reference signals use the same upsampling filters and keep
separate downsampling histories. The curve uses the integral of the preceding
sample again, and does not calculate it a second time. Until the first input
that is not zero, a drive skips silent frames that nothing has changed. After
the input starts, each frame goes through the processor, so later silence
keeps the filter tails. These paths keep the sample values.

The playback Options can run the Drive curve and its ADAA reference at the base
rate. This mode keeps the matching, focus, Mix, Output, and DC-removal stages.
A plain delay keeps the same 69-frame integer alignment, so a quality switch
does not bypass Drive. Audible chains and separate key chains use the same
explicit playback configuration. The default is the 4x path.

The oversampling delays the copy by `drive::LATENCY` (69 frames). Thus a chain
with a drive reads its voices, Repeat lane, and EQ that many frames ahead, and
the output of the drive aligns with the song. The track itself goes through a
plain delay of the same length. Thus a mix of 0% at 0 dB output is bit for bit
the track. A read that does not continue the last read first runs the drive
over the 69 frames from its first frame, because the other histories start from
silence there. Thus a seek leaves no gap. During loop playback, those source
reads stop at the loop end and fill the remaining lookahead with silence. Thus
sound outside the loop cannot get into the final frames of the loop.

The history of each drive, a key chain's history included, is in the
`RenderState`, in a slot that the mix gives it. `Frames::prepare_state`
allocates the slots before playback gives the source to the device. A drive
edit is a track edit and prepares a new mix, as an EQ edit does.

## Channel EQ and live track levels

`src/eq.rs` owns `ChannelEq`, the saved settings, and `EqParam`, each control
with its range. A mix designs the EQ curve of a chain one time, from the shared
RBJ biquads of the master board (`master::dsp`). An EQ edit is a track edit, so
it prepares a new mix, and playback continues on that mix.

A mix keeps the track levels that it was prepared with, and export uses them.
Playback reads the levels live from the `MasterControl` instead. It glides from
the level that it has reached to a new level over `GLIDE_FRAMES` (1024 frames,
about 23 ms), across the playback blocks. Thus an edit of only faders, pans,
polarity, or the master gain is not a render change, undo and redo included.

Each track publishes its signed gain and its pan knob as one atomic pair. Each
prepared mix applies the knob only where that mix has no pan lane. Thus, when a
lane is added or cleared, the playing mix keeps its pan behavior until its
replacement is ready, also when a fader moves during the rebuild. Playback
cannot mix channels from different edits. `MasterControl` also stores the
master gain in those levels, for the keys of the lane duckers.

A sequence counter covers the publication of all pairs and the master gain.
Playback makes one snapshot attempt for each block. If a writer overlaps the
attempt, playback keeps its last complete snapshot, or, for its first block,
the levels of the prepared mix. The audio thread does not wait for a lock and
does not try again. The master gain glides with the levels. Key normalization
divides by that gain directly. Thus a very small positive gain cannot overflow
a reciprocal and leave the detector of a ducker at infinity. Playback continues
without a click. An export prepares a new mix only when the levels are
different. Automation values are calculated at each output frame, so short
moves do not depend on the block size of the reader.
