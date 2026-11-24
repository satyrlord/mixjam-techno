# Drive and channel EQ

## Drive

The drive comes after the EQ and before the compressor. Thus the EQ chooses
what the curve bends. A low cut keeps rumble out of the curve. A boost drives
one band harder. The compressor after the drive makes the driven sound more
even. It hears the harmonics that the drive added, so its detector follows the
track as it sounds. Like the compressor, the drive runs before the fader, so
the fader does not change how hard the drive works. The drive also runs before
the automation and the sends. Thus they carry its harmonics.

The drive (`drive::Run`) does these steps:

1. It bends a copy of the track through its curve. Second-order
   antiderivative anti-aliasing (ADAA2) in `f64` acts on each sample. A
   straight line takes the same ADAA2 path, `(x0 + x1 + x2) / 3`, as the
   reference of the clean track.
2. It matches the power of the copy to the power of the track. Below about
   -55 dBFS the match relaxes to unity gain (`MATCH_GATE_POWER`), so quiet
   tails do not pump.
3. It high-passes the difference between the copy and the reference at its
   focus.
4. It adds the share of that difference that Mix sets to the track.
5. It holds the result at the power of the track, with the same gate, before
   the output trim. The two matching gains change at every eighth frame.

The render quality selects the oversampling path:

- Export, stems, and song-tool renders (`RenderQuality::Offline`) run the
  curve and the reference at 8 times the mix rate, through linear-phase
  halfband FIR filters (`FirOversampler`). These filters delay the copy by
  `drive::LATENCY` (71 frames). The track passes through a plain delay of the
  same length. Thus the chain reads its voices, Repeat lane, and EQ 71 frames
  ahead, and the output of the drive aligns with the song.
- Playback (`RenderQuality::Live`) at the default `2x` runs the curve and the
  reference at twice the mix rate, through polyphase IIR halfband filters
  (`StereoUpsampler2x` and `StereoDownsampler2x`, the two halves of
  `HalfBandIir2x` for both channels). The curve reads its antiderivatives
  from a cubic table. With `Off`, the curve runs at the mix rate. Both
  playback paths have no lookahead (`Run::latency` is 0), and the track
  itself has no delay.

A drive processes up to 32 frames at a time, one step at a time: the
oversampling, the curve, the reference, the blockers and gains, the focus,
and the output level. Each step goes through all the frames before the next
step starts. Thus the processor can work on many frames in parallel, and each
filter state gets the same values in the same order as with one frame at a
time. The output does not change by a bit.

A Mix of 0% at 0 dB output gives back the track bit for bit in each mode.

In playback, the copy and the reference lag the track: by the ADAA2 delay of
one sample at the processing rate, and at `2x` also by the phase of the IIR
filters. The difference that Mix adds has the same lag. Export has only the
ADAA2 lag of one eighth of a frame.

Before the first input that is not zero, a drive skips the silent frames,
because they change nothing. After the input starts, the processor handles each
frame, so later silence keeps the filter tails. The recursive filters guard
against denormal values at the end of each block. A test checks that the size
of a block does not change a bit of the output, also over a long silent tail.

If a read does not continue the last read, `drive::State::reset` clears the
history in place. In export, the drive then runs over the 71 frames from the
first frame of the read. Thus a seek leaves no gap.

The `RenderState` holds the history of each drive, a key chain's history
included, in a slot that the mix gives it. `Frames::prepare_state` allocates
the slots before playback gives the source to the device. A drive edit is a
track edit and prepares a new mix, as an EQ edit does.

## Channel EQ and live track levels

`src/eq.rs` owns `ChannelEq`, the saved settings, and `EqParam`, each control
with its range. A mix designs the EQ curve of a chain once, using biquads. The
cut bands share a 12 or 24 dB/octave slope. The window and Inspector plot the
response of the same filters. An EQ edit prepares a new mix, and playback
continues on that mix. The EQ runs the two channels of a frame through each
biquad side by side (`StereoBiquad`), with the same operations for each
channel. The compressor processes up to 64 frames at a time in passes, as the
drive does. The output does not change by a bit.

A mix keeps the track levels from its preparation, and export uses them.
Playback reads the levels live from the `MasterControl` instead. It glides from
the level that it has reached to a new level over `GLIDE_FRAMES` (1024 frames,
about 23 ms), across the playback blocks. Thus an edit of only faders, pans,
polarity, or the master gain is not a render change. This includes undo and
redo.

Each track publishes its signed gain and its pan knob as one atomic pair. Each
prepared mix applies the knob only where that mix has no pan lane. Thus the mix
that plays keeps its pan behavior until its replacement is ready. This holds
when an edit adds or clears a lane, and when a fader moves during the rebuild.
Playback cannot mix channels from different edits. `MasterControl` also stores
the master gain in those levels, for the keys of the lane duckers.

A sequence counter covers the publication of all pairs and the master gain.
Playback makes one snapshot attempt for each block. If a writer overlaps the
attempt, playback keeps its last complete snapshot. For its first block, it
uses the levels of the prepared mix. The audio thread does not wait for a lock
and does not try again. The master gain glides with the levels. Key
normalization divides by that gain directly. Thus a very small positive gain
cannot overflow a reciprocal and leave the detector of a ducker at infinity.
Playback continues without a click. Export also prepares a mix again when the
levels are different. The mix calculates automation values at each output
frame. The Filter lane is the exception: it sets its cutoff at every eighth
frame of the song. Thus short moves do not depend on the block size of the
reader.
