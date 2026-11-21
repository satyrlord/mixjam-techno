# Playback and the prepared mix

## Contents

These parts have their own page,
[Voices and sample reading](voices-and-sample-reading.md):

- [Voices, reversal, and the Pitch
  lane](voices-and-sample-reading.md#voices-reversal-and-the-pitch-lane)
- [Band-limited sample
  reading](voices-and-sample-reading.md#band-limited-sample-reading)

## Playback preferences

Before an Apply of the Audio preferences, playback, pending playback, previews,
and recording must stop. Workspace and Files changes do not need a stopped
transport. A playback-quality change makes the prepared mixes invalid and
advances the render revision. Thus an older worker result cannot replace the
new quality. `src/playback_quality.rs` supplies the Drive quality, the master
saturation quality, and the sample read quality. These three values are
explicit and independent, and they are outside `Project` and `MasterChain`.

Exports always prepare at full quality, also when a playback mix at reduced
quality exists. A mix prepared at reduced quality refuses to export. The
preview gain applies only to previews. The meter display throttle keeps the
peaks between displayed readings and does not change the audio analysis.

## Track levels and polarity

Track gain is a nonnegative amplitude. Polarity is a separate persistent
boolean. Playback and export use `Track::effective_gain()` to apply the sign.
The Mixer shows the gain as `20 * log10(amplitude)` dB, and zero amplitude is
negative infinity. The model admits track and master gains from 0 through 4.
New Mixer edits usually stop at +12 dB (about 3.9811 amplitude). An exact
track value of 4 in a saved song stays the same through load and focus. Clip
gain is 0 through 2.

A polarity change ends a gain or pan gesture and makes a discrete undo step.
Silence keeps the polarity. Unity changes only the amplitude. L/R, Mute, Solo,
the channel EQ switch, and the channel EQ Reset are also discrete undo steps.
`Track::audible()` applies Mute and Solo for playback and export. When a track
is soloed, only the soloed tracks play. Mute has priority over Solo.

## The prepared mix

`audio::Mix` is a project that is prepared for rendering: its clips are
resolved to output frames, with their decoded sources. It mixes any range of
the song on demand. The player reads it in blocks on the audio thread, without
allocation or locks. Export writes it block by block. Thus neither of them
holds a buffer of the whole song, and only the timeline limits the song length.
The mix itself is dry.

`Project::sample_playback_bpm` chooses the nearest tempo octave for factual
source tempos. MixJam v7 sources keep their saved-span timing instead. Their
tempo was calculated to fit the saved duration, not measured from a loop.
The adapter's `MixJam` group, together with its v7 source-controls notice,
identifies these imported sources, also in saved projects. Missing sources
keep the explicit `mixjam-span` timing policy, and conflicts keep their source
spans. Rendering, new placement, and clip handles use this same rule.

Playback, render, and the sample editors read sample audio through
`Library::read_pcm`, which hides a stray end. Many original product samples end
on one or two frames that jump to a large value. `Library::read_pcm` gives the
value of the frame before them to these frames:

- The last frames of a channel that are far above all frames in the 10 ms
  before them. If the frames just before them are near silence, twice as loud
  is sufficient.
- A single last frame whose step is more than four times each step in that
  window.

Sample identity and digests read `Library::read_original_pcm`, which keeps the
data as stored.

## Output device and the audio thread

`Player` owns a cpal stream through `src/audio/output.rs`. The stream callback
plays the device mixer of the application (`src/audio/device_mixer.rs`).
Options select Automatic, 20 ms, or 40 ms (the default). A fixed target
requests the related frame count at the current device rate. A known
unsupported range rejects the target. If a target fails at startup, the
startup can use the system default and gives an explicit fallback reason. If
that buffer also fails, the error keeps both causes. If the default output
device does not open, the fallback tries the other devices, not the failed
default again. Its reason names the failure and the device in use.
Apply opens a silent replacement on the same device and format while playback
is stopped. It keeps the old stream until the settings persistence succeeds,
and it rejects opening failures. The output status shows the requested frames
separately from the observed callback frames and the last measured callback
interval. CPAL does not expose the negotiated buffer capacity, so that value is
not available.

`audio::flight` keeps the latest 30 seconds of device and mixer samples in
preallocated atomic rings. The output callback writes to them without
allocation or locks. A worker examines fresh audio for sustained loud
broadband noise. It also records non-finite samples and stream errors,
startup faults included. An automatic capture usually waits four seconds, and
there is a maximum of one each five minutes. An output replacement saves its
pending capture early. A process exit can stop the worker before a pending save
is complete.

`Session` publishes the song and transport context a maximum of once each
second. While an output exists, it polls a minimum of once each 250 ms, also
while idle. F12, the Options action, and the agent `capture` request use the
same Session operation to save the rings and the diagnostic state under the
data folder. A capture records the callback output, not the downstream driver
or speaker audio.

The device controls the WASAPI shared-mode callback cadence. A 40 ms request
does not promise 40 ms callbacks. The device mixer reads `PlaybackSource` one
frame at a time inside the device callback. The callback that gets to the end
of a block renders the next block in its own period. Thus playback renders
short blocks of `PlaybackSource::BLOCK_FRAMES` (256 frames, 5.8 ms), and a
callback renders one or more blocks for each device request. Export uses its
own 65,536-frame blocks.

The level glides, FX tails, and meters keep their own 1024-frame timing.
Playback uses the same block schedule as export (`src/audio/schedule.rs`), so
changing the block size leaves the tail boundaries and samples intact.
Playback from the start at full quality, with fixed settings and no loop,
matches the export samples before PCM16 encoding and device conversion.

On its first read on a thread, a playback source asks the system one time for
each thread to run that thread as the "Pro Audio" task of the multimedia class
scheduler. If that is not possible, it asks for the highest priority of the
process class. cpal 0.15.3 also makes this request, but it gives a thread id
where the call needs a handle, so the system refuses the request. If the
request fails, playback continues and the window reports the failure one
time.

The callback never locks, waits, allocates, or frees:

- The window prepares each playback as a voice. It allocates the voice on its
  own thread. It gives the voice to the callback through a wait-free ring of
  preallocated slots, with a single producer (`src/audio/handoff.rs`). At the
  start of its next buffer, the callback moves the waiting voices into a fixed
  set of eight places.
- Each voice shares its stop switch, pause, volume, and played frames with the
  `Playback` handle of the window as atomics. Thus play, stop, pause, seek (a
  new playback), loop regions, and preview gain need no message and no lock.
  The callback reads the volume and the pause once for each buffer.
- When a new playback replaces a playback, the old handle's stop switch is set
  before the new voice goes to the callback. The old voice plays the remainder
  of its block beside the new voice, up to 5.8 ms, and renders no other block.
  A paused voice that is stopped ends immediately.
- Ended voices go back through a second ring to the retired-playback thread,
  which frees them each 20 ms. A replaced mix can hold hundreds of megabytes
  of samples, and to free 309 MB takes about 26 ms. If the ring is full, the
  callback keeps the voice for the next buffer. If that thread does not exist,
  the callback frees the voice. The master pedals of a source stay with it
  after its last frame, so that thread also frees them.
- The mixer converts each voice to the device rate. It reads along a straight
  line between source frames, with the source-to-device ratio reduced to whole
  numbers, so the blocks have no seam. A mono source plays on the first two
  channels, a mono device takes the left channel, and other channels are
  silent. The mixer adds the voices into a buffer that it allocates when it is
  made. It then writes the sample format of the device, writes a non-finite
  sample as silence, and gives each sample to the flight recorder.
- The settings of the master pedalboard and of the FX buses get to playback
  through `audio::Latest`, a lock-free latest-value cell. The window writes a
  slot that no reader holds and publishes it. The audio thread pins the
  published slot and copies it. Only writers take turns, under a lock that no
  reader uses. Track levels, tail clears, meters, and compressor histories are
  atomics.

The `device_mixer` tests guard these rules. A counting allocator asserts that
the callback does not allocate or free after warm-up. The test covers steady
playback, live edits of the board, the buses, and the levels, a replacement, a
loop, a pause, the end of a song, and the silence after it, into float and
16-bit buffers. A scan of the callback modules asserts that they hold no lock,
wait, or channel, and that the master control holds no `try_lock`.
