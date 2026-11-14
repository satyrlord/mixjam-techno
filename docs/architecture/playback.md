# Playback and the prepared mix

## Playback preferences

Audio preferences require playback, pending playback, previews, and recording
to stop before Apply. Workspace and Files changes do not require a stopped
transport. A playback-quality change invalidates prepared mixes and advances
the render revision, so older worker results cannot replace the new quality.
`src/playback_quality.rs` supplies explicit independent Drive, master
saturation, and sample read quality outside `Project` and `MasterChain`.
Exports prepare full quality explicitly, including when a reduced-quality
playback mix exists, and a mix prepared at reduced quality refuses to export.
Preview gain applies to previews only. Meter display throttling retains
peaks between displayed readings and does not change audio analysis.

## Track levels and polarity

Track gain is a nonnegative amplitude and polarity is a separate persistent
boolean. Playback and export use `Track::effective_gain()` to apply the sign.
The Mixer presents `20 * log10(amplitude)` dB; zero amplitude is negative
infinity. Track and Master model admission is 0 through 4, while new
Mixer edits normally stop at +12 dB (about 3.9811 amplitude). Earlier exact
track values of 4 survive load and focus. Clip gain remains 0 through 2.
Changing polarity ends a gain or pan gesture and creates a discrete undo
step. Silence retains polarity; Unity changes only amplitude. L/R, Mute, and
Solo are discrete undo steps too, and so are the channel EQ's switch and
Reset. `Track::audible()` applies Mute and Solo
for playback and export: when any track is soloed, only soloed tracks play,
and Mute wins over Solo.

## The prepared mix

`audio::Mix` is a project prepared for rendering: its clips resolved to output
frames, with their decoded sources. It mixes any range of the song on demand.
The player reads it in blocks on the audio thread, without allocation or locks,
and export writes it block by block, so neither holds a buffer of the whole
song and song length is bounded only by the timeline. The mix itself is dry.

`Project::sample_playback_bpm` chooses the nearest tempo octave for factual
source tempos. MixJam v7 sources instead keep their saved-span timing: their
tempo was calculated to fit the saved duration, not measured from a loop.
The adapter's `MixJam` group together with its retained v7 source-controls
notice identifies these imported sources, including saved projects. Missing
sources retain the explicit `mixjam-span` timing policy; conflicts retain
their source spans. Rendering, new placement, and clip handles share this rule.

Playback, render, and the sample editors read sample audio through
`Library::read_pcm`, which hides a stray end: many original product samples
end on one or two frames that jump to a large value, and the last frames of a
channel that are far above everything in the 10 ms before them (or twice as
loud when the frames just before them are near silent) take the value before
them, and so does a single last frame whose step is over four times any step
in that window. Sample identity and digests read `Library::read_original_pcm`,
which keeps the data as stored.

## Output device and the audio thread

`Player` owns a cpal stream through `src/audio/output.rs`, whose callback
plays the app's own device mixer (`src/audio/device_mixer.rs`). Options select Automatic, 20 ms, or 40 ms
(default). Fixed targets request the corresponding frame count at the existing
device rate. A known unsupported range rejects the target. Startup can use the
system default if a target fails, with an explicit fallback reason. Apply opens
a silent replacement on the same device and format while stopped. It retains
the old stream until settings persistence succeeds and rejects opening failures.
The output status reports requested frames separately from observed
callback frames and the last measured callback interval. CPAL does not expose
negotiated buffer capacity; that value remains unavailable.

`audio::flight` keeps the latest 30 seconds of device and mixer samples in
preallocated atomic rings. The output callback writes them without allocation
or locks. A worker checks fresh audio for sustained loud broadband noise and
observes non-finite samples and stream errors, including startup faults.
Automatic captures normally wait four seconds and are limited to one per five
minutes. Replacing an output saves its pending capture early. Process exit
can end the worker before a pending save completes.
`Session` publishes song and transport context at most once per second and
polls at least every 250 ms while an output exists, including while idle.
F12, the Options action, and the agent `capture` request use the same Session
operation to save the rings and diagnostic state under the data folder.
Captures record the callback's output, not downstream driver or speaker audio.

WASAPI shared-mode callback cadence remains device-controlled; requesting
40 ms does not promise 40 ms callbacks. The device mixer reads
`PlaybackSource` a frame at a time inside the device's callback, and the
callback that reaches the end of a block renders the next one
within its own period. Playback therefore renders short blocks,
`PlaybackSource::BLOCK_FRAMES` (256 frames, 5.8 ms), so a callback renders one
or more according to the device's request; export keeps its own
65,536-frame blocks. Nothing audible depends on playback's block size: the
level glides, the FX tails, and the meters keep their own 1024-frame timing,
so playback in blocks of any size gives export's samples. On its first read
on a thread, a playback source asks the system once per thread to run that
thread as the multimedia class scheduler's "Pro Audio" task, or else at the
highest priority of the process's class. cpal 0.15.3 asks too, but passes a
thread id where the call needs a handle, so the system refuses it. If the
request fails, playback goes on and the window reports it once.

The callback never locks, waits, allocates, or frees:

- The window prepares each playback as a voice, allocated on its own thread,
  and hands it to the callback through a wait-free single-producer ring of
  preallocated slots (`src/audio/handoff.rs`). The callback takes waiting
  voices at the start of its next buffer into a fixed set of eight places.
- Each voice shares its stop switch, pause, volume, and played frames with the
  window's `Playback` handle as atomics, so play, stop, pause, seek (a new
  playback), loop regions, and preview gain need no message and no lock. The
  callback reads volume and pause once a buffer.
- When a new playback replaces one, the old handle's stop switch is set
  before the new voice is handed over. The old voice plays out the rest of its
  block beside the new one, up to 5.8 ms, and renders no other block. A paused
  voice that is stopped ends at once.
- Ended voices go back through a second ring to the retired playback thread,
  which frees them every 20 ms: a replaced mix can hold hundreds of megabytes
  of samples, and freeing 309 MB takes about 26 ms. A full ring keeps the
  voice for the next buffer; if that thread is gone, the callback frees it.
  A source's master pedals stay with it after its last frame, so they too are
  freed on that thread.
- The mixer converts each voice to the device's rate by reading along a
  straight line between source frames, with the source-to-device ratio reduced
  to whole numbers, so blocks leave no seam. A mono source plays on the first
  two channels, a mono device takes the left, and other channels stay silent.
  It sums the voices into a buffer allocated with the mixer, then writes the
  device's sample format, with a non-finite sample written as silence, and
  hands each sample to the flight recorder.
- The master pedalboard's settings and the FX buses' settings reach playback
  through `audio::Latest`, a lock-free latest-value cell: the window writes a
  slot that no reader holds and publishes it, and the audio thread pins the
  published slot and copies it. Only writers take turns, under a lock that no
  reader touches. Track levels, tail clears, meters, and compressor histories
  were atomics already.

The `device_mixer` tests guard this. A counting allocator asserts that the
callback neither allocates nor frees after warm-up, through steady playback,
live edits of the board, the buses, and the levels, a replacement, a loop, a
pause, the end of a song, and the silence after it, into float and 16-bit
buffers. A scan of the callback's modules asserts that they hold no lock,
wait, or channel, and that the master control holds no `try_lock`.

## Voices, reversal, and the Pitch lane

`Frames::read_block` with a sends buffer gives the dry sum together with the
feeds of the four FX buses, eight interleaved channels a frame: each voice adds
its sample, after the track's gains and pan, times the track's send. A voice
applies its clip's fades per frame. A reversed clip's voice reads, for each
output frame, the source position of its mirror frame in the clip (`start +
end - 1 - frame`), so it plays the forward slice backwards as a pure function
of the frame, never of what played; its fades, gain, and pitched copy apply in
the clip's own time, and its sound ends with the clip. Being in the voice, a
reversal comes before every track stage, the Repeat lane included. The Pitch
lane is in the voice too: `automation::Varispeed` turns the track's lane into
a clock, the integral of the speed `2^(cents / 1200)` over the output frames,
with a closed form on each straight segment in cents. A voice of that track
reads its source at the clock's distance from the clip's start (reversed,
back from the clip's last frame), times the tempo step, instead of at the
frame's distance, so the read moves at the lane's speed without a jump and
is still a pure function of the frame. Preparation finds where the clock
has run through the source, to end the clip's sound and its fade out there.
The lane scales the read rate of the clip's pitched copy, so clip pitch and
lane add in cents, and it comes before the Repeat lane, which replays the
slowed sound.

## Band-limited sample reading

A voice that plays off its source's own sample rate or tempo, or under a
Pitch lane, reads its source between frames. `src/audio/resample.rs` reads it
there band-limited, with a Kaiser-windowed sinc kernel (beta 8.5, cutoff 0.49
of the source rate, 48 taps at speeds up to 1). The kernel is one table of 128
points per source frame from its centre to its edge, each with the step to the
next point, so a tap is a straight-line read of the kernel. Each tap's table
position steps outwards from the read in fixed point. At a speed above 1
the cutoff falls by the speed and the span widens by the same factor, so the
content above the output's Nyquist frequency goes before it can fold back.
The gain uses that same rounded tap spacing, and tap sums use double
precision to retain steady levels during long reads. Preparation rejects a
speed when its tap spacing rounds to zero, including the maximum saved Pitch
value. Very high representable speeds can still require substantial CPU;
the ordinary-rate spectral and cost measurements do not cover those workloads.
The spectral tests cover 44.1 and 48 kHz sources at steps 1.047, 1.088,
0.9, and 1.14. For the tested tones within the kernel's pass band, residual
images and aliases stay at least 80 dB down and gain is within 0.01 dB to
17 kHz. These bounds do not cover the transition band near source Nyquist,
all source rates, or every playback speed. Slowing near-Nyquist content can
move both its attenuation and its residual images into the audible band;
low-rate WAVs can expose this at lower frequencies. The kernel is symmetric
and centred on the read, so it adds
no delay: each frame reads the position that the straight-line read did, and a
clip that starts at frame N sounds at N. The read hears only the clip's slice
of the source, from its offset. It holds the slice's first frame before the
slice and the source's last frame after the source, as the straight-line read
held its last frame. The clip's start, trim, end, and fades keep their frames,
and no read reaches past the source. Under a Pitch lane the cutoff follows the
lane's mean speed over the 32 frames on each side of the frame. That mean is a
function of the frame alone, so a jump in speed moves the cutoff over 64 frames
instead of at once, and playback from any frame and in any block size reads
what export reads. A reversed voice reads the same positions backwards, so it
remains a pure function of the source. A voice at its source's own rate and
tempo without a Pitch lane reads whole frames and copies them. `Mix::new`
builds the table with `resample::prepare` before playback reaches the audio
thread, and the audio thread only reads it. On 2026-11-03, the optimized
`sample_read_cost` test measured 172 to 189 ns per frame with the kernel and
21 ns with the straight-line read. Under a Pitch lane, which also reads the
lane's clock three times per frame, it measured 306 to 307 ns against 66 to
67 ns. The test used Windows, an Intel Core i7-11700, mono and stereo bright
sources, 44.1 kHz output, three fixed rate/tempo ratios, and a -3 to +3
semitone Pitch lane. Each case read 20 seconds in 512-frame blocks. These
voice costs do not establish a full-song or physical-device deadline.
Live playback can choose the straight-line
read with the Sample playback quality option, Low. Exports, track exports,
and song-tool renders prepare at `PlaybackQuality::FULL` and always read
band-limited.
