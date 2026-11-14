# Track chains and render state

## Sample rates

The engine mixes at one rate, `audio::Mix::SAMPLE_RATE` (44.1 kHz). The
master DSP rate, the FX lookahead, fade, tail and sleep lengths, the count-in,
and the preferred device rate derive from it. The native sample format has
its own constant, `legacy_format::NATIVE_SAMPLE_RATE`. Product import,
library admission, and the fallback for a source of unknown rate use it.
The two rates are equal today, but the code keeps them as separate values.

## Track chains

`prepare_project` decodes and checks each
source once, and makes one copy per clip pitch off the audio thread with
`sample_edit::repitch`; both steps share the sources out among all but one of
the processor's cores. The render worker keeps each copy until no clip uses that
pitch. A track whose channel EQ, drive, or compressor is on, whose width is
not 100%, that has automation, whose ducker is on, or that keys a ducker is a
`TrackChain`: it mixes its voices into a 128-frame stack buffer, which its
Repeat lane, its EQ (`src/eq.rs`), its drive (`src/drive.rs`), its compressor
(`src/compressor.rs`), its width, then its filter and volume lanes
(`src/automation/`), its ducker (`src/ducker.rs`), its level, its pan lane,
and its send lanes shape before the sum. A track
whose pan lane has points leaves its pan knob out of its level, and the lane
applies the same balance law at each frame, so key levels, sends, and stems
follow the moving pan. The Repeat lane (`automation::Repeat`) comes first, on
the clips' sound, so its repeats go through the EQ, drive, compressor, width,
filter, volume, fader, pan knob or lane, ducker, and FX sends. It does not
record what played: it maps each output frame to a frame of the voices, which
are a pure function of the frame. Where the lane turns on at tick T, every
retrigger reads the voices again from T, one division long, so the slice
comes from the clips at T whether or not playback passed T. A retrigger, and
the switch off, crossfade over `REPEAT_RAMP_FRAMES` (132 frames, 3 ms): the
outgoing slice plays on under the incoming one. The Repeat gate lane weighs
each tap: a retrigger with gate G at its own tick keeps weight 1 for G
percent of its division's frames, then falls over the same ramp to 0, and the
outgoing tail of a crossfade keeps its own retrigger's gate weight; at 100%
every weight is unchanged, bit for bit. Playback started inside a
run, export, and stems therefore read the same repeats; the stages after it
keep their own histories as for any sound. A key track's own chain repeats
too, so a ducker hears its key's repeats. The mix runs the chains chunk by
chunk: each key track first renders alone in a chain of its own, with its own
filter histories, and leaves its peak level per frame in a stack array (at
most `MAX_KEYS`); then each chain adds its chunk, and a ducked chain
reads its key's levels for the same frames. A key is the key track as it
sounds after its fader, before its own ducker, so duckers never chain; a
muted key is silent. A lane ducker reads its key's level without the master
gain, so the master gain does not change how far it ducks: the mix divides
the key's peak by the master gain, gliding as the levels glide, and a master
gain of 0 leaves the key silent. The key audio for the master pedals and the
FX buses keeps the master gain, as their input does. A track without a
ducker computes exactly as before.
The same key chains serve the master pedals' and the FX buses' keys
(`Project::side_keys`): while any listens, the mix keeps each key's stereo
audio for the block in its `RenderState`, and `Frames::keys` hands it, as
`ducker::Keys`, to `ReturnsProcessor::process_keyed` and
`MasterProcessor::process_keyed`. The board gives the keys its trim and
delays each pedal's key by the pedals that run before it, so the key lines
up with the sound. Key delay lines advance through silence while a pedal uses
its own input, so switching back cannot replay old key audio. `PlaybackSource`
allocates key scratch for a full block before it is sent to the audio device.
A master or FX edit that changes the required set of keys prepares the mix
again; other master and FX edits reach playback live. The outputs are already in each voice; the track's
level (fader, polarity, pan, and master gain) comes in at mix time, in the
voice for a plain track and after the volume lane for a chain. The
compressor and the width act on the track's own sound before its level: the
compressor follows the mean power of both channels with the master Bus
compressor's curve (`master::dsp::reduction`), so the fader does not change
how hard it works, and the width scales the side of each frame, on a track
whose outputs are both on. After them every stage is linear and per channel
up to the ducker, whose gain follows the separate key, so this order sounds
as the signal order clips (read by the Pitch lane), Repeat lane, EQ, drive,
compressor, width,
automation, fader and pan, ducker, sends. A track with none of these mixes its voices straight into the
sum, so an EQ, drive, or compressor that is off, or a width of 100%, leaves the
render bit for bit as it was.

## Drive

The drive comes after the EQ and before the compressor. The EQ then chooses
what is driven, such as a low cut that keeps rumble out of the curve or a
boost that drives one band harder. The compressor after it evens out the
driven sound and hears the harmonics the drive added, so its detector follows
the track as it sounds. Like the compressor, the drive works before the
fader, so the fader does not change how hard it drives, and before the
automation and the sends, which therefore carry its harmonics. The drive
(`drive::Run`) bends a copy of the track through its curve at four times the
mix rate, with antiderivative anti-aliasing on each sample, matches the
copy's power to the track's, high-passes the difference between the copy and
the track at its focus, adds the mix's share of it to the track, and holds the
result at the track's power before the output trim. The driven and reference
signals share their identical upsampling filters and keep separate
downsampling histories. The curve reuses the preceding sample's integral
instead of calculating it again. Until the first nonzero input, a drive skips
untouched silent frames. After input starts, every frame runs through the
processor so later silence retains the filter tails. These paths preserve
the sample values.
Playback Options can run the Drive curve and its ADAA reference at the base
rate instead. This mode retains the matching, focus, Mix, Output, and DC-removal
stages. A plain delay keeps the same 69-frame integer alignment; switching
quality does not bypass Drive. Audible chains and separate key chains use the
same explicit playback configuration. The default 4x path remains unchanged.
Its oversampling delays
the copy by `drive::LATENCY` (69 frames), so a chain with a drive reads its
voices, Repeat lane, and EQ that many frames ahead and the drive's output
lines up with the song. The track itself goes through a plain delay of the
same length, so a mix of 0% at 0 dB output is bit for bit the track. A read that does not
continue the last one first runs the drive over the 69 frames from its first
frame, as the other histories start from silence there, so a seek leaves no
gap. During loop playback, those source reads stop at the loop end and fill
the remaining lookahead with silence, so sound outside the loop cannot leak
into its final frames. Each drive's history, a key chain's too, lives in the `RenderState` in a
slot that the mix gives it; `Frames::prepare_state` allocates the slots
before playback hands the source to the device. A drive edit is a track edit
and prepares a new mix, as an EQ edit does.

## Channel EQ and live track levels

`src/eq.rs` owns `ChannelEq`, the saved settings, and `EqParam`, each
control with its range; a mix designs a chain's EQ curve once
from the master board's shared RBJ biquads (`master::dsp`). An EQ edit is a
track edit, so it prepares a new mix and playback resumes on it. A mix keeps
the track levels it was prepared with, which export uses. Playback reads them
live from the `MasterControl` instead, and glides from the level it has
reached to a new one over `GLIDE_FRAMES` (1024 frames, about 23 ms), across
playback's blocks, so an edit of faders, pans, polarity, or the
master gain alone is not a render change, including undo and redo. Each
track publishes its signed gain and pan knob as one atomic pair. Each prepared
mix applies the knob only where that mix has no pan lane. Adding or clearing
a lane therefore keeps the playing mix's pan behavior until its replacement
is ready, including when a fader moves during the rebuild. Playback cannot
mix channels from different edits. `MasterControl` also stores the
master gain inside those levels for the lane duckers' keys. A sequence
counter covers publication of all pairs and the master gain. Playback makes
one snapshot attempt per block. If a writer overlaps it, playback retains
its last complete snapshot, or the prepared mix's levels for its first block.
The audio thread neither waits for a lock nor retries. Master gain glides
with the levels. Key normalization divides by that gain
directly, so very small positive gains cannot overflow a reciprocal and
leave a ducker's detector at infinity. Playback continues without a
click, and an export prepares a new mix only when the levels differ.
Automation values are evaluated at each output frame, so short moves do not
depend on the reader's block size.

## Render state and compressor history

The shared mix is immutable. Each playback or export owns a `RenderState` with
the EQ, compressor, and filter histories of each chain and the next-frame
cursor, passed to `Frames::read_block`. A read that does not continue the
previous one, after a seek or a loop, starts the histories from silence, with
one exception: where playback continues, its compressors carry on. A
compressor that started without reduction would play its track up to its
full reduction louder until the attack caught up. Two cases carry on:

- A loop pass keeps each compressor's detector power and reduction from the
  loop's end in the `RenderState`, which knows the loop region.
- A new playback continues the last one: after a pause, or on a mix prepared
  again during playback, such as for each step of a compressor dial. After
  each block, playback keeps each compressed track's power and reduction in
  its `MasterControl`, with the frame after that block. A new playback that
  starts at most `CARRY_FRAMES` (4096 frames) before that frame starts the
  track's compressor from them. That covers the frames playback renders
  ahead of what has played, the master pedals' delay, and the cursor's tick
  and update lag.

The kept history follows the track, not the chain's place in the mix. A key
chain takes its track's history too, so a lane ducker, a master pedal, or an
FX bus hears its compressed key as the track sounds. Every other start, such
as a seek, a Play from another bar, another song, or a compressor switched
back on later, begins from silence, as does a start at the song's start,
which follows silence as in export. Loop wraps retain history even when the
loop starts at frame zero. Explicit seeks, stops, and project replacement
invalidate kept histories. Compressor power changes invalidate that track's
history, including undo and redo, without resetting other tracks. Each audio
source retains its start generation; an older source cannot restore an
invalidated history. A history from another passage would
leave the track quieter than export, and a lane it keys louder, for up to the
release time. Export reads straight through from the song's start with no
`MasterControl`, so it never uses a kept history and stays deterministic.
