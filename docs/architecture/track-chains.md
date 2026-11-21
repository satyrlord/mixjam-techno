# Track chains and render state

## Contents

These parts have their own page,
[Drive and channel EQ](drive-and-channel-eq.md):

- [Drive](drive-and-channel-eq.md#drive)
- [Channel EQ and live track
  levels](drive-and-channel-eq.md#channel-eq-and-live-track-levels)

## Sample rates

The engine mixes at one rate, `audio::Mix::SAMPLE_RATE` (44.1 kHz). It sets the
master DSP rate, the FX lookahead, the fade, tail, and sleep lengths, the
count-in, and the preferred device rate. The native sample format has its own
constant, `legacy_format::NATIVE_SAMPLE_RATE`. Product import, library
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
- It has automation outside the Compression lane.
- It is the key track of a ducker, a master pedal, or an FX bus.

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

If the pan lane of a track has points, the level of the track does not include
the pan knob. The lane applies the same balance law at each frame. Thus key
levels, sends, and stems follow the pan lane.

### The Repeat lane

The Repeat lane (`automation::Repeat`) comes first, on the sound of the clips.
Thus its repeats pass through the EQ, drive, compressor, width, filter, volume,
fader, pan knob or lane, ducker, and FX sends. It does not record what played.
It maps each output frame to a frame of the voices, which are a pure function of
the frame. Where the lane changes from off to on at tick T, each retrigger reads
the voices again from T, for one division. Thus the slice is the sound of the
clips at T, whether or not playback passed T. A crossfade over
`REPEAT_RAMP_FRAMES` (132 frames, 3 ms) covers each retrigger and the change to
off. The outgoing slice continues under the incoming slice.

The Repeat gate lane gives a weight to each tap. A retrigger with gate G at its
own tick keeps weight 1 for G percent of the frames of its division. The weight
then falls to 0 over the same ramp. The outgoing tail of a crossfade keeps the
gate weight of its own retrigger. At 100%, each weight stays the same, bit for
bit. Thus playback that starts inside a run, export, and stems read the same
repeats. The stages after the lane keep their own histories, as for any sound.
The chain of a key track also repeats, so a ducker hears the repeats of its key.

### Keys and duckers

The mix runs the chains chunk by chunk:

1. Each key track renders alone in a chain of its own, with its own filter
   histories. It leaves its peak level for each frame in a stack array (a
   maximum of `MAX_KEYS`).
2. Each chain adds its chunk. A ducked chain reads the levels of its key for
   the same frames.

A ducker that is on never keys its own track. `Ducker::validate` rejects such a
song. `Ducker::set_key` stops a dial from setting such a key. The Inspector and
the channel EQ window both set the key through it.

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
key aligns with the sound. Key delay lines advance through silence while a pedal
uses its own input, so a switch back cannot replay old key audio.
`PlaybackSource` allocates key scratch for a full block before it goes to the
audio device. A master or FX edit that changes the necessary set of keys
prepares the mix again. Other master and FX edits reach playback live.

### Levels and signal order

The outputs are already in each voice. The level of the track (fader, polarity,
pan, and master gain) applies at mix time. For a plain track it applies in the
voice. For a chain it applies after the volume lane. The compressor and the
width operate on the sound of the track before its level:

- The compressor follows the mean power of the two channels with the curve of
  the master Bus compressor (`master::dsp::reduction`). Thus the fader does
  not change how hard it works. The Compression lane, when the track has
  one, mixes each compressed frame with the frame before the compressor. The
  compressor only scales each frame, so the mix stays in phase. The detector
  runs at every value of the lane.
- The width scales the side of each frame, on a track whose two outputs are
  on.

After them, each stage is linear and operates on each channel separately, up to
the ducker. The gain of the ducker follows the separate key. Thus this signal
order sounds the same:

- clips (read by the Pitch lane)
- Repeat lane
- EQ, drive, compressor, and width
- automation
- fader and pan
- ducker
- sends

A track without these stages mixes its voices directly into the sum. Thus a
track with its EQ, drive, and compressor off and a width of 100% renders bit for
bit the same.

## Render state and compressor history

The shared mix is immutable. Each playback or export owns a `RenderState` that
`Frames::read_block` receives. The `RenderState` holds the EQ, compressor, and
filter histories of each chain and the next-frame cursor. A read that does not
continue the previous read starts the histories from silence. This happens after
a seek or a loop. One exception exists: where playback continues, its
compressors continue. A compressor that starts without reduction plays its track
louder until the attack catches up. The difference is up to the full reduction.
A compressor continues in two cases:

- The `RenderState` knows the loop region. A loop pass keeps the detector power
  and the reduction of each compressor from the end of the loop in it.
- A new playback continues the last playback. This happens after a pause, and
  when the application prepares the mix again during playback, for example for
  each step of a compressor dial. After each block, playback keeps the power and
  the reduction of each compressed track in its `MasterControl`, with the frame
  after that block. A new playback that starts at most `CARRY_FRAMES` (4096
  frames) before that frame starts the compressor of the track from these
  values. This distance covers the frames that playback renders before they
  play. It also covers the delay of the master pedals and the tick and update
  lag of the cursor.

The kept history follows the track, not the place of the chain in the mix. A
key chain also takes the history of its track. Thus a lane ducker, a master
pedal, or an FX bus hears its compressed key as the track sounds.

All other starts begin from silence. Examples are a seek, a Play from another
bar, another song, and a compressor that goes from off to on again later. A
start at the start of the song also begins from silence, as in export. Loop
wraps keep the history, also when the loop starts at frame zero. Explicit seeks,
stops, and project replacement make kept histories invalid. A change of the
compressor power makes the history of that track invalid, also by undo and redo.
It does not reset other tracks. Each audio source keeps its start generation, so
an older source cannot restore a history that is invalid.

A history from another passage can make the track quieter than export, and a
lane that it keys louder, for up to the release time. Export reads directly
through from the start of the song with no `MasterControl`. Thus it never uses a
kept history, and its result is deterministic.
