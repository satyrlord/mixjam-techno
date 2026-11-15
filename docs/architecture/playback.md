# Playback and the prepared mix

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
startup can use the system default and gives an explicit fallback reason.
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

No audible result depends on the playback block size. The level glides, the FX
tails, and the meters keep their own 1024-frame timing. Thus playback in
blocks of any size gives the same samples as export.

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

## Voices, reversal, and the Pitch lane

`Frames::read_block` with a sends buffer gives the dry sum together with the
feeds of the four FX buses, eight interleaved channels for each frame. Each
voice adds its sample, after the gains and pan of the track, multiplied by the
send of the track. A voice applies the fades of its clip for each frame.

A reversed clip's voice reads, for each output frame, the source position of
the mirror frame in the clip (`start + end - 1 - frame`). Thus it plays the
forward slice backwards, as a pure function of the frame and never of what
played. Its fades, its gain, and its pitched copy apply in the clip's own
time, and its sound ends with the clip. Because the reversal is in the voice,
it comes before each track stage, the Repeat lane included.

The Pitch lane is also in the voice. `automation::Varispeed` turns the lane of
the track into a clock. The clock is the integral of the speed
`2^(cents / 1200)` over the output frames, with a closed form on each straight
segment in cents. A voice of that track reads its source at the clock's
distance from the start of the clip, multiplied by the tempo step. A reversed
voice reads back from the last frame of the clip. Thus the read moves at the
lane's speed without a jump, and it is still a pure function of the frame.

Preparation finds where the clock has gone through the source, and ends the
sound and the fade out of the clip there. The lane scales the read rate of
the pitched copy of the clip, so the clip pitch and the lane add in cents. The
lane comes before the Repeat lane, which replays the slowed sound.

## Band-limited sample reading

A voice reads its source between frames when it plays off the sample rate or
the tempo of its source, or under a Pitch lane. `src/audio/resample.rs` reads
there band-limited, with a Kaiser-windowed sinc kernel: beta 8.5, cutoff 0.49
of the source rate, and 48 taps at speeds up to 1. The kernel is one table of
128 points for each source frame, from its centre to its edge. Each point
holds the step to the next point, so a tap is a straight-line read of the
kernel. The table position of each tap steps outwards from the read in fixed
point.

At a speed above 1, the cutoff falls by the speed and the span becomes wider
by the same factor. Thus the content above the Nyquist frequency of the output
is removed before it can fold back. The gain uses that same rounded tap
spacing. Tap sums use double precision to keep steady levels during long
reads. Preparation rejects a speed when its tap spacing rounds to zero, the
maximum saved Pitch value included. Very high speeds that the format can hold
can still need much CPU. The spectral and cost measurements at usual rates do
not cover those workloads.

The spectral tests cover 44.1 kHz and 48 kHz sources at steps 1.047, 1.088,
0.9, and 1.14. For the test tones in the pass band of the kernel, residual
images and aliases stay a minimum of 80 dB down, and the gain is within
0.01 dB up to 17 kHz. These bounds do not cover the transition band near the
source Nyquist frequency, all source rates, or all playback speeds. When
playback slows near-Nyquist content, its attenuation and its residual images
can move into the audible band. WAVs with a low rate can show this at lower
frequencies.

The kernel is symmetric and centred on the read, so it adds no delay. Each
frame reads the same position as the straight-line read, and a clip that
starts at frame N sounds at N. The read uses only the slice of the source that
the clip plays, from its offset. Before the slice, it holds the first frame of
the slice. After the source, it holds the last frame of the source, as the
straight-line read does. The start, trim, end, and fades of the clip keep their
frames, and no read goes past the source.

Under a Pitch lane, the cutoff follows the mean speed of the lane over the 32
frames on each side of the frame. That mean is a function of the frame only.
Thus a jump in speed moves the cutoff over 64 frames, not immediately, and
playback from any frame and in any block size reads the same as export. A
reversed voice reads the same positions backwards, so it is still a pure
function of the source. A voice at the rate and tempo of its source, without a
Pitch lane, reads whole frames and copies them. `Mix::new` builds the table
with `resample::prepare` before playback gets to the audio thread. The audio
thread only reads the table.

On 2026-11-03, the optimized `sample_read_cost` test measured 172 to 189 ns
for each frame with the kernel, and 21 ns with the straight-line read. Under a
Pitch lane, which also reads the lane clock three times for each frame, it
measured 306 to 307 ns against 66 to 67 ns. The test conditions were:

- Windows, an Intel Core i7-11700, and 44.1 kHz output.
- Bright mono and stereo sources, three fixed rate-to-tempo ratios, and a -3
  to +3 semitone Pitch lane.
- 20 seconds of reading for each case, in 512-frame blocks.

These voice costs do not establish a deadline for a full song or a physical
device. Live playback can use the straight-line read when the Sample playback
quality option is Low. Exports, track exports, and song-tool renders prepare
at `PlaybackQuality::FULL` and always read band-limited.
