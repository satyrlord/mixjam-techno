# Track chains and render state

## Sample rates

The engine mixes at one rate, `audio::Mix::SAMPLE_RATE` (44.1 kHz). The master
DSP rate, the FX lookahead, the fade, tail, and sleep lengths, the count-in,
and the preferred device rate come from it. The native sample format has its
own constant, `legacy_format::NATIVE_SAMPLE_RATE`. Product import, library
admission, and the fallback for a source of unknown rate use it. The two rates
are equal, but the code keeps them as separate values.

## Track chains

`prepare_project` decodes and checks each source one time. Off the audio
thread, it makes one copy for each clip pitch with `sample_edit::repitch`.
Both steps divide the sources among all the cores of the processor but one.
The render worker keeps each copy until no clip uses that pitch.

A track is a `TrackChain` when one of these conditions is true:

- Its channel EQ, drive, compressor, or ducker is on.
- Its width is not 100%.
- It has automation.
- It is the key of a ducker.

A `TrackChain` mixes its voices into a 128-frame stack buffer. These stages
then shape the buffer, in this order, before the sum:

1. The Repeat lane.
2. The EQ (`src/eq.rs`).
3. The drive (`src/drive.rs`).
4. The compressor (`src/compressor.rs`).
5. The width.
6. The filter and volume lanes (`src/automation/`).
7. The ducker (`src/ducker.rs`).
8. The level.
9. The pan lane.
10. The send lanes.

If the pan lane of a track has points, the level of the track does not
include the pan knob. The lane applies the same balance law at each frame.
Thus key levels, sends, and stems follow the moving pan.

### The Repeat lane

The Repeat lane (`automation::Repeat`) comes first, on the sound of the clips.
Thus its repeats go through the EQ, drive, compressor, width, filter, volume,
fader, pan knob or lane, ducker, and FX sends. It does not record what played.
It maps each output frame to a frame of the voices, which are a pure function
of the frame. Where the lane turns on at tick T, each retrigger reads the
voices again from T, for one division. Thus the slice comes from the clips at
T, whether or not playback went through T. A retrigger, and the switch off,
crossfade over `REPEAT_RAMP_FRAMES` (132 frames, 3 ms). The outgoing slice
continues under the incoming slice.

The Repeat gate lane gives a weight to each tap. A retrigger with gate G at its
own tick keeps weight 1 for G percent of the frames of its division. The weight
then falls to 0 over the same ramp. The outgoing tail of a crossfade keeps the
gate weight of its own retrigger. At 100%, each weight is unchanged, bit for
bit. Thus playback that starts inside a run, export, and stems read the same
repeats. The stages after the lane keep their own histories, as for any sound.
The chain of a key track also repeats, so a ducker hears the repeats of its
key.

### Keys and duckers

The mix runs the chains chunk by chunk:

1. Each key track renders alone in a chain of its own, with its own filter
   histories. It leaves its peak level for each frame in a stack array (a
   maximum of `MAX_KEYS`).
2. Each chain adds its chunk. A ducked chain reads the levels of its key for
   the same frames.

A key is the key track as it sounds after its fader and before its own ducker.
Thus duckers never chain, and a muted key is silent. A lane ducker reads the
level of its key without the master gain, so the master gain does not change
how much it ducks. The mix divides the peak of the key by the master gain, and
the division glides as the levels glide. A master gain of 0 makes the key
silent. The key audio for the master pedals and the FX buses keeps the master
gain, as their input does. The key stages do not change the output of a track
without a ducker.

The same key chains serve the keys of the master pedals and of the FX buses
(`Project::side_keys`). While one of them listens, the mix keeps the stereo
audio of each key for the block in its `RenderState`. `Frames::keys` gives that
audio, as `ducker::Keys`, to `ReturnsProcessor::process_keyed` and
`MasterProcessor::process_keyed`. The board applies its trim to the keys. It
delays the key of each pedal by the pedals that run before that pedal, so the
key aligns with the sound. Key delay lines advance through silence while a
pedal uses its own input, so a switch back cannot replay old key audio.
`PlaybackSource` allocates key scratch for a full block before it goes to the
audio device. A master or FX edit that changes the necessary set of keys
prepares the mix again. Other master and FX edits get to playback live.

### Levels and signal order

The outputs are already in each voice. The level of the track (fader,
polarity, pan, and master gain) comes in at mix time: in the voice for a plain
track, and after the volume lane for a chain. The compressor and the width
operate on the sound of the track before its level:

- The compressor follows the mean power of the two channels with the curve of
  the master Bus compressor (`master::dsp::reduction`). Thus the fader does
  not change how hard it works.
- The width scales the side of each frame, on a track whose two outputs are
  on.

After them, each stage is linear and operates on each channel separately, up
to the ducker, whose gain follows the separate key. Thus this order sounds the
same as this signal order: clips (read by the Pitch lane), Repeat lane, EQ,
drive, compressor, width, automation, fader and pan, ducker, sends.

A track without these stages mixes its voices directly into the sum. Thus an
EQ, drive, or compressor that is off, or a width of 100%, keeps the render bit
for bit the same.

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

## Render state and compressor history

The shared mix is immutable. Each playback or export owns a `RenderState` that
`Frames::read_block` receives. The `RenderState` holds the EQ, compressor, and
filter histories of each chain and the next-frame cursor. A read that does not
continue the previous read, after a seek or a loop, starts the histories from
silence, with one exception: where playback continues, its compressors
continue. A compressor that starts without reduction plays its track louder,
by up to its full reduction, until the attack catches up. There are two cases
where a compressor continues:

- A loop pass keeps the detector power and the reduction of each compressor
  from the end of the loop in the `RenderState`, which knows the loop region.
- A new playback continues the last playback: after a pause, or on a mix that
  is prepared again during playback, for example for each step of a compressor
  dial. After each block, playback keeps the power and the reduction of each
  compressed track in its `MasterControl`, with the frame after that block. A
  new playback that starts a maximum of `CARRY_FRAMES` (4096 frames) before
  that frame starts the compressor of the track from these values. That
  covers the frames that playback renders before they play, the delay of the
  master pedals, and the tick and update lag of the cursor.

The kept history follows the track, not the place of the chain in the mix. A
key chain also takes the history of its track. Thus a lane ducker, a master
pedal, or an FX bus hears its compressed key as the track sounds.

All other starts begin from silence: for example, a seek, a Play from another
bar, another song, or a compressor that is switched on again later. A start at
the start of the song also begins from silence, as in export. Loop wraps keep
the history, also when the loop starts at frame zero. Explicit seeks, stops,
and project replacement make kept histories invalid. A change of the
compressor power makes the history of that track invalid, undo and redo
included, but it does not reset other tracks. Each audio source keeps its
start generation, so an older source cannot restore a history that is invalid.

A history from another passage would make the track quieter than export, and a
lane that it keys louder, for up to the release time. Export reads directly
through from the start of the song with no `MasterControl`. Thus it never uses
a kept history, and its result is deterministic.
