# Application boundary

The Rust library is the backend. The native executable contains the frontend.
`src/application.rs` is the frontend's only application API. It has no egui,
window, file-dialog, or screenshot dependency.

```text
Native views and input                  Headless application
src/ui/view.rs + src/ui/stretch.rs       src/application.rs
             + src/ui/effects.rs
             + src/ui/eq.rs
             + src/ui/step_synth.rs
             + src/ui/dialog.rs
             + src/ui/controls.rs
             + src/ui/sample_editor.rs
             + src/ui/mixer.rs
             + src/ui/master.rs
             + src/ui/fx.rs
             + src/ui/agent.rs
             + src/ui/album_review.rs
             + src/ui.rs
        |                                       |
        | immutable project/catalog views       | Editor and history
        | typed edits and service calls         | Library and project files
        +-------------------------------------->| Render queue and transport
                                                | Recording and sample processing
```

## Ownership

`Session` owns the editor, catalog, project file, dirty state, render revisions,
audio player, microphone capture, and pending recorded take. Its project and
catalog accessors are read-only. It validates the project when it starts and
after each change. Typed `EditCommand` values change tempo, loop regions, clips,
routing, sections, and mixer settings. Separate operations place and move clips.
Successful edits update history and, when needed, mark the render for update. An accepted command that
changes nothing is not an edit: it leaves history, the saved state, and the render
unchanged. Changes to tempo or one gain, pan, send, channel EQ dial, FX knob, or clip control that arrive within 400 ms
of each other form one history entry, so a slider drag is one undo step. Saving
ends that group, so undo can return to the saved state after the next edit. A clip
cannot be lengthened over the next clip on its track. Automation point drags
use explicit start and end boundaries, so a pause within a drag does not split
its undo step and separate drags do not merge.
Clip handle drags keep a frontend preview and submit one `edit_discrete` action
on release. This ends continuous-control grouping before and after the edit,
so separate drags and agent `clip` requests each keep their own undo step.

Sections are ordered named timeline markers. `SetSections` replaces the list
as one validated history step. Section edits and their undo/redo mark the song
modified but preserve the render revision, prepared mix, and pending playback.
The native UI owns section rename drafts. `Session` owns bookmark visibility
as an application option; the Sections toolbar and Options window use the same
validated preference API. Visibility does not modify the song. Section positions extend
the scrollable arrangement without extending the audio render.

`src/options.rs` owns the typed application preference schema, defaults,
validation, and persistence in `options.json` under `project_io::data_root()`.
`Session` owns the active values, their revision, and `apply_options`.
`src/ui/options.rs` owns one editable window draft and page selection. Apply
checks the revision, validates the complete draft, and prepares a replacement
output when needed before one atomic file replacement. Active values change
only after the write succeeds. Cancel and close ask before discarding pending
changes; a clean draft closes immediately. Restore options defaults changes
the draft only. No-op Apply performs no write or
audio rebuild. Options do not enter song history or modify song data.

Missing options import only the existing section-bookmark visibility. The old
file remains intact and receives no further writes. Invalid files load safe
defaults with a visible error and need explicit Restore options defaults before
replacement. A future schema remains intact and blocks preference writes.
Library configuration and the last-used song folder retain their own storage.
`WorkspaceLayout` in `src/options.rs` stores the Tools, Boards, and Sounds
switches and the Maximise state in `layout.json` beside `options.json`. `Session` adopts each
layout change at once and saves it for the next launch. A failed write reports
an error but keeps the current layout. Headless sessions do not save layouts.
Layout changes do not change the Options revision, song data, or history.
`src/peaks.rs` holds read-only waveform peaks for library sounds: the lowest and
highest sample of every 64 frames, all channels together, and coarser tables
of four times as many frames each until a few buckets remain. Views ask
`Session::sample_peaks(id)`, which answers `Pending`, `Ready`, or `Unavailable`
and never decodes on the UI thread. The first request queues the sound for one
lazily started worker thread, which reads it through `Library::read_pcm`, takes
the newest request first, and drops requests from an older catalog. The tables
stay until the catalog revision changes. `Session::poll` collects finished
tables and keeps the view refreshing while any are pending.
`src/ui/sounds.rs` draws the Sounds panel from those tables and shares the
filters and sound actions with the lower panel's Sounds module.
`src/ui/waveform.rs` turns a ready table into one column pair per point for an
arrangement clip, following the trim, Reverse, the fades, and where the sound
ends, and paints the visible columns as one mesh. `src/ui/view.rs` keeps these
shapes per clip and rebuilds one only when its key changes: the sound, trim,
length, Reverse, fades, points per tick (zoom), frames per tick (tempo),
catalog revision, or peak table. The catalog revision prevents a replacement
sound from reusing a stale waveform when its peak table reuses an address.
`src/ui/lanes.rs` owns the Arrange lane header's layout constants, the lane
name derived from the first clip's sound family, and the lane fader.
`src/ui/libraries.rs` renders library management as the Sample libraries page of
Options. Folder scans and provider changes still use their existing Session
operations and storage. A successful scan adopts its folder changes; fallback
BPM is used by Add library folder and Scan again. The options draft and its
Cancel action cannot commit, stop, or roll back these operations. There is no separate library window.
Untouched startup Options may close after its initial scan fills the library;
manual navigation, a preference draft, or Restore defaults keeps it open.
New fixed song folders must be absolute existing directories when applied. A folder
that later becomes unavailable remains configured; song dialogs report it and
use the last-used/default location. Unrelated preferences and no-op Apply remain
available while that configured folder is unavailable.

Audio preferences require playback, pending playback, previews, and recording
to stop before Apply. Workspace and Files changes do not require a stopped
transport. A playback-quality change invalidates prepared mixes and advances
the render revision, so older worker results cannot replace the new quality.
`src/playback_quality.rs` supplies explicit independent Drive, master
saturation, and sample read quality outside `Project` and `MasterChain`.
Exports prepare full quality explicitly, including when a reduced-quality
playback mix exists, and a mix prepared at reduced quality refuses to export.
Preview gain applies at the preview sink only. Meter display throttling retains
peaks between displayed readings and does not change audio analysis.

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

Project copies share one imported-sample list until an edit changes it, so a
history entry or render request costs the same for any palette size.

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

`Player` owns a cpal stream through `src/audio/output.rs` and attaches rodio
sink queues to its dynamic mixer. Options select Automatic, 20 ms, or 40 ms
(default). Fixed targets request the corresponding frame count at the existing
device rate. A known unsupported range rejects the target. Startup can use the
system default if a target fails, with an explicit fallback reason. Apply opens
a silent replacement on the same device and format while stopped. It retains
the old stream until settings persistence succeeds and rejects opening failures.
`output_configuration` reports requested frames separately from observed
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
40 ms does not promise 40 ms callbacks. rodio pulls
`PlaybackSource` one sample at a time inside the device's callback, and the
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
request fails, playback goes on and the window reports it once. Each source
plays inside a `Playing` wrapper. When a new playback replaces it, its stop
switch is set before the new source is appended, and the old source ends
before its next block, so no callback renders both. When the sink lets go of
it on the audio thread, the wrapper hands the source through a bounded queue
to the retired playback thread, which frees it: a replaced mix can hold
hundreds of megabytes of samples, and freeing 309 MB takes about 26 ms.

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

`prepare_project` decodes and checks each
source once, and makes one copy per clip pitch off the audio thread with
`sample_edit::repitch`; both steps share the sources out among all but one of
the processor's cores. The render worker keeps each copy until no clip uses that
pitch. A track whose channel EQ, drive, or compressor is on, whose width is
not 100%, that has automation, whose ducker is on, or that keys a ducker is a
`TrackChain`: it mixes its voices into a 128-frame stack buffer, which its
Repeat lane, its EQ (`src/eq.rs`), its drive (`src/drive.rs`), its compressor
(`src/compressor.rs`), its width, then its filter and volume lanes
(`src/automation.rs`), its ducker (`src/ducker.rs`), its level, its pan lane,
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
and prepares a new mix, as an EQ edit does. `src/eq.rs` owns `ChannelEq`, the saved settings, and
`EqParam`, each control with its range; a mix designs a chain's EQ curve once
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

`src/fx.rs` owns the buses:
`FxBuses` is the saved settings, `DelayParam` and `ReverbParam` name each
continuous control with its range, and `ReturnsProcessor` runs the buses on
the feeds and adds their returns to the dry sum, before the master pedalboard.
Its delay lines are sized and allocated when the processor is built, so the
audio thread never allocates, including when a limiter resets. A bus's return
lane (`Project::return_automation`, resolved by the mix into `ReturnLanes`)
replaces its return level frame by frame: the mix passes the lanes and the
block's song frame to `ReturnsProcessor::process_at` and `split`, and the bus
multiplies its effect's output by the lane's straight-line value just before
its safety limiter, read `LOOKAHEAD` frames ahead so that each returned frame
carries the lane's value at its own time. The song frame keeps counting
through the tail, in export and playback alike, so a lane that moves after
the song's end shapes the tail the same way. The lane only scales the return;
the effect's state runs on whatever the lane does, as it does behind a zero
Return knob. A bus with no
input and no remaining internal tail sleeps; delay sleep allows enough time
for a pending long echo. Bypass does not hide internal tail activity from this
check. The safety limiter's fixed 220-frame lookahead comes off the delay's
output tap, while feedback keeps the full selected time, so every repeat stays
on the beat. Reverb compensates its pre-delay where possible. Export runs the stage on silence after the song
while any bus rings, for at most `TAIL_LIMIT`, in 1024-frame blocks
(`Mix::TAIL_BLOCK`). Playback also asks whether a bus still rings once a tail
block, however short its own blocks are, so export and playback give the same
samples.
If an FX type change completes inside a processing block, its sidechain key
changes on that same frame. Processing the same inputs in different block
sizes must not delay the new key until the next block.
`src/master.rs` owns the master pedalboard: `MasterChain` is the saved
settings, `Knob` names each continuous setting with its range, and
`MasterProcessor` runs the board over the summed stereo mix in blocks, with
state that carries from block to block. Its submodules hold the Booster
pedals' DSP (`booster`), this application's mastering pedals (`pedals`), their shared
filters, oversampling, and true-peak interpolation (`dsp`), and the playback
meters (`meters`). `MasterChain` is `Copy` and its `order` a fixed array that
names each pedal once, so a settings copy never allocates on the audio thread,
and the whole board state is fixed-size. Pedals that are off pass the signal on
unchanged. Each pedal keeps its own state when the order changes. Master
processors prepare shared oversampling and true-peak filter tables before
playback reaches the audio thread. First playback and later pedal switches
must not initialize or wait for those tables in the callback. The mastering
playback quality option changes only Soft clip, Tube, Maximizer, and Tape.
Off runs their saturation at the base rate with the same integer stage delay.
It does not change limiter lookahead, true-peak interpolation, metering,
other processors, or the fixed live delay. The mastering
pedals glide to new settings over about 20 ms. During playback, switching or
moving a pedal copies the board and crossfades from the old copy to the new one
over 30 ms. Live playback pads both paths to a fixed 593-frame delay and fills
the new path before the fade starts, so switching pedals preserves their
timeline alignment and the song's ending. `MasterProcessor` applies the trim,
then holds the mix to full scale, then runs the pedals, so a lower trim keeps
a hot mix from clipping; the mix and the FX returns reach it unclamped, and a
playback without a board, or `Frames::read`, holds them instead.
`MasterChain::bypass` skips the trim
and every pedal: `MasterChain::runs` is false for every pedal, the trim glides
to unity, and the board's latency is zero, so export carries no pedal delay.
During playback the bypassed board still pads to the fixed delay and meters,
and switching the bypass crossfades as a pedal switch does. Leaving bypass
starts the pedals that are on from a clear state.
`MasterChain::latency` is the delay of the pedals that run:
`Mix::to_pcm` and `Mix::write_wav` run the pedals past the end by that much and
cut it from the start. Playback drops its fixed delay before the first sample
and drains the same number of frames at the end, so both stay on the timeline.
Export and `Mix::to_pcm` take the project's chain
explicitly. Mix playback reads a `MasterControl` that the
`Session` keeps equal to the project; the audio thread takes new settings at
its next block with `try_lock`, so it never waits. An edit or history step that
changes only the pedalboard or the FX buses keeps the prepared mix and playback
when the set of required key tracks stays the same. A changed set of keys
prepares the mix again. The same `MasterControl` carries the FX buses'
settings and Clear Tail requests, which the `ReturnsProcessor` polls the same way. Tool previews play without the pedalboard. Each
playback and export starts the pedalboard in the same settled state. A live
processor also measures: every 1024 frames it publishes a `MeterReading` through
atomics in the `MasterControl`, which `Session::master_meters` reads without
waiting. Each playback numbers its readings, and only the start of a playback
changes that number. A stopped playback that is still finishing its last block
can show its levels for one block, but it cannot keep the next playback's readings
from reaching the window. The cumulative OVER count carries its playback number
in the same atomic value. A stale count cannot become the new playback's lamp
baseline. Readings change more than 30 times a second, and each covers the
same frames whatever playback's block size, so the block size changes neither
the true peak shown nor the OVER count. At the end, playback publishes the
frames since the last reading and finishes the peak detector without adding
audio frames or loudness time. Output metering excludes the initial 593 frames
that playback removes, so loudness windows follow the audible program.
Export measures nothing. Stereo PCM16 export reserves 36 bytes of the RIFF
size field for its header before it opens a destination file.
The Multiband's moving minimum retains exactly the active lookahead window,
including when gain requirements rise for longer than its fixed queue. It
expires old entries before adding a new one, so a full queue cannot overwrite
its live minimum.
`Mix::write_stems` exports the premaster in parts, one pass over the song:
each sounding track mixes alone through its own `RenderState`, its sends
summed into the buses' feeds, and `ReturnsProcessor::split` gives each bus's
return apart, without clamping. Parts follow `render`'s blocks and tail rule,
so every file is as long as the mix export, and they skip the master
pedalboard. PCM16 stem export rejects a non-finite or out-of-range part before
publishing files, instead of clipping each part separately. The parts sum to
the bypassed mix when that mix does not clip. `atomic_file::write_many` stages
every file before it replaces any. The render worker prepares
the mix, keeps the decoded sources of the last one, and discards them when the
library is replaced. With audio output, the `Session` asks it for each new
revision as soon as a song opens or a render edit lands, so Play finds the mix
ready; a revision whose preparation fails waits for Play, which asks again and
reports the error. `Session::start_export` renders and writes on its own
thread and reports through `poll`; `Session::export` stays synchronous for
headless callers. `Session::start_track_export` and `Session::export_tracks`
do the same for the parts. One export of either kind runs at a time.
`Session::catalog_revision` advances when the library is
replaced; the frontend keeps its filtered palette until then. Folder adoption
takes providers that are already loaded from the current library and reads a
provider from disk only when its folder is gone.
When a saved provider replaces a partial product during song opening, it
keeps already loaded sample identities that the saved provider does not have.
When an original provider is already loaded, opening also checks the song's
saved WAV folder for used native IDs that provider lacks. It supplements only
those IDs and keeps the current provider's identities and the saved binding.
If the used native WAVs of one product cannot fit under one saved folder,
`project_io::save_with_library` retains their exact file bytes together under
`native-providers/<product>/<content hash>` in the data folder. It verifies
the native IDs, decoded audio and timing before saving that binding. Repeated
saves reuse verified content. A retention failure leaves the existing song
unchanged. This keeps native identities without changing the song format.
WAV path resolution includes verified duplicate copies that the palette omits.
MixJam v7 suffix relocation and native WAV lookup share this complete path
inventory, so a reference to a duplicate still resolves after its folder moves.
Format 18 records one provider per product. A song using both a packed original
provider and supplemental WAVs for that product cannot be represented by that
binding. Save rejects the combination before replacing the song.

A song queued behind a folder scan records both the render revision and the
editor generation. Any intervening project edit cancels that deferred open,
including a master-only edit followed by Save. Master-only edits still retain
the prepared dry mix. Opening a project adopts freshly measured WAV metadata,
including changes to length and channel count at the same path. History changes
can still reuse existing metadata without reading each source again.
The agent's Choose path request for Open also records this edit identity. If
the user edits the song while that dialog is collapsed, or creates a recorded
take, confirming the old request keeps the current song and path draft. The
user must cancel and start Open again. Changes approved before the request do
not prevent it from opening the selected song.
Adding or removing an optional sample provider is unavailable while recording
or scanning sample folders. Rejection preserves the current libraries, scan,
recorded take, project, and settings.

`src/atomic_file.rs` owns file replacement. Projects, settings, the scan cache,
synth patterns, and WAV exports are staged beside the destination and renamed
over it. A failed single-file write leaves the existing file intact. A stem
batch rejects duplicate or non-file destinations, stages all files, and keeps
the originals as sibling backups until every replacement succeeds. A commit
failure restores the originals; if restoration fails, the error names the
retained backups for recovery. A process interruption can leave a partial batch
and backups. The batch is not an atomic filesystem transaction.

MIX import caches resolved paths and audio metadata. Only the pending asset list
owns retained audio bytes, and distinct payloads share its 500 MB limit. Saving
a song reads the previous document header through a bounded stream to preserve
its creation time. The completed serialized song must fit the reader's 1 GiB
limit before it can replace the destination.

The frontend owns selection, drag gestures, zoom, layout, open windows, search
filters, and tool form drafts. It converts pointer positions into requested
positions and sends them to the application. It does not own an `Editor` or
change the live project. It does not decode audio, write projects, or control a
device directly.
Replacing a song or adopting scanned folders invalidates held control gestures
in Mixer, Mastering, Effects, and the channel EQ. Movement from an earlier
gesture cannot edit the replacement song.

`src/ui/theme.rs` owns the three application themes: each theme's color roles,
its material (flat, plastic, or metal), corner radii, and fonts, and the code
painters for plastic surfaces, gradients, flat and molded dials, and fader
caps. Each frame sets a thread-local current theme from the option;
`theme::adopt` rebuilds the egui style and fonts only when the theme changes.
Renderers read roles, and only shared painters branch on the material;
interaction and parameter code never depend on the theme.

`src/ui/dialog.rs` owns the shared tool shell and lifecycle for all
in-app tools, utility windows, and blocking dialogs. It owns header layout,
position, edge resizing, open/collapsed geometry, focus layers, stable identities,
and accessible modal naming. The caller owns the task and decides what an action
or cancellation means. Native operating-system windows and file pickers are outside
this shell.

`src/ui/controls.rs` owns reusable control presentation and interaction: buttons,
header icons, rotaries, sliders, numeric values, switches, and the scrolling
status text of tool footers. Roles and states
share the same rendering and keyboard/accessibility behavior. Rotary full travel
is 150 logical points; Shift applies fine adjustment to new movement. Parameter
specifications retain ranges, reset values, logarithmic mapping, and discrete
choices. Tool bodies own content and drafts, without independent window skins or
button implementations.

The shared chassis uses subtly aged folded painted metal, dark inset control
bays, cream lettering, polished silver knobs, and pale metal primary buttons.
`controls::Palette` supplies Utility, Marine, Granite, Red, Olive, and Indigo variants.
Time Stretcher uses Marine; Sample Editor uses Granite; Effect Studio uses Red;
Step synth uses Olive; Mastering uses Indigo. Utilities use neutral metal with orange-yellow accents.
Prepared material and control PNGs live in `assets/folded-metal/<palette>/{100,150,200}/` and the shared
`assets/folded-metal/controls/{100,150,200}/`.
The chassis contains no decorative screws or fasteners. Its continuous
antialiased perimeter and bevel are native geometry. Raster material fades before
the outer outline and at strip ends to avoid visible joints. Knob edges use 8x
supersampling and one premultiplied-alpha Lanczos reduction per density.
Fixed details fade to transparent over a matching solid chassis color. Resizing
extends the solid regions; painting aligns one texel to one physical pixel. Text
and input remain live, and content reflows or scrolls instead of scaling artwork
or controls. Palette variants share geometry and interaction.

Collapse retains position, expanded size, and task state. Collapsed blocking
dialogs release workspace and agent input without executing their pending action.
`src/ui.rs` owns blocker precedence, paused save continuation, and guards against
replacement of pending file requests. Choose path suspends its parent confirmation
until the path request resolves. A close action cancels a confirmation, without
implicitly saving or discarding. Recorded take can collapse while its unsaved
in-memory take remains available for explicit Save take or Discard take.
A deferred file or close action checks recording again when it resumes. If a
recording started while its confirmation was collapsed, the action stays
pending and the recording remains intact until the user stops it.

Native file dialogs return paths for application operations. Screenshot encoding
is isolated presentation verification code.

Visual artwork uses transparent PNGs only. Embedded SVG artwork and SVG files
are prohibited in tracked and non-ignored project files.
`tools/assets/check-raster-assets.py` uses Git's file inventory and respects
ignore rules. Original archives and the Rust dependency
cache reside in `local-data/`; temporary files reside in `tmp/`. Runtime PNG textures
use prepared density variants with one texel per physical screen pixel; views
snap their bounds to the pixel grid and do not resize them.

Sample-processing forms hold drafts until the user requests playback or saving.
The application validates and processes those drafts. It also owns synth-pattern
file parsing, generated audio registration, import resolution, and settings files.
`src/ui/sample_editor.rs` owns the Sample Editor body and native action responses.
It uses the shared shell and controls for frame fields, pitch slider, buttons,
header, and feedback. Numeric formatting does not change the draft.

Frame drafts retain unsigned 64-bit values, with an inclusive start and exclusive end. Pitch keeps the
existing -24 to +24-semitone UI range. Numeric display rounding does not alter
the draft. Background selections do not reset trim while the editor is collapsed.
On expansion, a changed active source ID or catalog revision applies the existing
trim reset. Expanded source-change behavior remains unchanged, so a replaced
source cannot retain stale bounds.
`src/ui.rs` adapts Listen edit, Save edited sound, and Import details. Existing
Session trim and resampling remain synchronous. Preview uses existing PCM
playback; save uses the existing WAV workflow, registers the result in Wave
at the source BPM, and selects it. Neither the panel nor the artwork performs
file or audio operations. Missing sources and invalid ranges disable processing
with explicit tooltip reasons; the footer shows range and duration or recovery
guidance.

`src/ui/stretch.rs` owns Time Stretcher's content and action responses inside the
shared shell. Source, tempo, processing, preview, and save feedback use shared
controls. Its Help action stays available in both open and collapsed states.

`src/ui.rs` adapts its source selections, Listen, Save, and Import details
requests to existing application operations; the panel does not render audio or
write files. Opening it from the tool rail initializes the original BPM draft
from the selected sample's source tempo.
The application owns Time Stretcher's background source read and processing job.
Each request captures its source catalog and tempo values, rejects duplicate
work, and retains one consumable result. Library or song replacement invalidates
that result. The frontend retains the requested action and rejects results after
the tool closes or its source, tempos, or catalog change. It opens the existing
save workflow or requests PCM preview only for a current result. Preview state
and a revision identify whether that tool's audition is still playing; a later
preview, transport change, or stop cannot inherit its active indicator.
Save completion updates the local feedback and the selected output's source
tempo; cancelling a file choice does not report success.

`src/ui/mixer.rs` owns Mixer's compact strip presentation in the current theme.
Its `gain_keys` and `gain_semantics` give the Mixer's gain rail and the lane
fader the same wheel, key, accessibility, and double-click behaviour.
Vintage tiles the graphite and ivory artwork; Studio and Aero paint the strips
and fader caps in code at the same density scale.
Each strip is 108 logical points wide. Master remains outside the horizontally
scrolling occupied-track bank. Each track strip's EQ button starts the row of
Mute and Solo, above polarity, and the Master strip's Bypass sits under its
output labels; both mirror project state. `src/ui/eq.rs` owns the channel EQ
window: the frontend keeps which track it shows, and the window edits copies
of that track's `ChannelEq`, `Drive`, `Compressor`, width, and `Ducker`, sent
as `EditCommand::SetMixer`. It reuses the
Mastering dials and paints its response curve. The Mixer's toolbar holds
Export tracks, which asks for a folder through `ask_path`. Native controls
paint live labels, dB values,
rail marks, pan, routing, polarity, focus, and hover. Gain uses a piecewise dB
taper with 72% of travel between -24 and +12 dB; the bottom section maps linear
amplitude to exact silence. Unity sits at 0 dB on the gain rail. Center aligns
with the pan midpoint just below its rail. Polarity has a separate
slashed-circle button. Control-specific tooltips and the Help window carry
detailed guidance.
Prepared material tiles and fader caps live in `assets/mixer/{100,150,200}/`.
The fader is first reduced to a 48 by 64-pixel canonical grid, then prepared
at 24 by 32, 36 by 48, and 48 by 64 pixels. Runtime painting aligns one texel
to one physical pixel, with no texture resizing. The empty bank gives one
instruction and leaves Master available.

`src/ui/fx.rs` owns the Effects board. It places the four buses in raised
units on a Marine surface, with the effect and preset choices, dials, return
level, switches, and Clear tail. `src/ui/view.rs` sends a changed bus set as
an edit and sends Clear tail to the session. The Mixer's return-strip Edit
button selects the module and brings that bus into view.

The frontend's `BottomModule` enum selects Sounds, Mixer, Mastering, or Effects. `src/ui/view.rs`
owns their shared 28-point header and fixed left-side `Bottom module` combo.
Its width, `BOTTOM_MODULE_WIDTH`, is 168 points, which shows every module name
in full; the selected module uses the theme's 16-point heading face.
The combo has bounded layout space and truncates long selected text; its
accessible value and popup retain the full name.
The combo keeps its control ID and horizontal anchor across module changes.
Sounds filters render inline to its right. Each module renders only its body
below that header. The Mixer and Mastering tool-rail shortcuts select their
module, and select Sounds when that module is already selected. Selection and
shared panel height are frontend session state. Switching preserves resized
height; opening Mixer, Mastering, or Effects raises a shorter panel to its
minimum: 400 points for Mixer, or the whole board for Mastering and Effects.
Each yields down to 370 points in a window too short to keep a 150-point lane viewport. Sounds
retains its 180-point minimum.
Later modules can extend the enum and the same selector.

`src/ui/effects.rs` owns Effect Studio's sections and action responses. It uses
shared rotaries, value entry, switches, action buttons, header, and status styling.
Keep labels, values, and dial marks separate. Verify rendered bounds at all three
prepared densities; source coordinates alone do not establish visual acceptance.

`src/ui.rs` adapts Listen, Save sound, Reset, Help, and Import details. Listen
and Save sound call `Session::processed_pcm` synchronously; Listen then uses the
existing PCM preview. Save uses the existing file request and
`Session::save_pcm`, registers the WAV in Effect at the source BPM, and selects
the saved sound. Source changes clear local feedback; save choice, cancellation,
failure, and success update it. Reset restores `EffectsState::default`.
Turning an optional section off discards its settings; enabling it uses that
section's defaults. This presentation does not change the algorithms or the
shared `effects_controls` form used by Step synth. It does not add a background
processing job.

`src/ui/master.rs` owns the Mastering module of the lower panel, its
preset selector, and its Bypass all switch: a horizontal pedalboard in
signal order. The Input unit (trim and VU meter) stays at the left and the
Output unit (loudness, true peak, OVER) at the right; between them the pedal
row scrolls, joined by painted cables. A strip below names every pedal in
order, outlines those in view, and scrolls the row to the one chosen; keyboard
focus scrolls a control into view too. Each pedal shows its name, lamp, dials,
arrow buttons, and switch, and the dynamics pedals their gain-reduction lamps;
a pedal that is off is dimmed. Dragging a pedal's header moves it, with an
insertion marker while it is dragged, and the row scrolls near its ends; the
arrow buttons give the same move to the keyboard, screen readers, and agents.
Project or library replacement advances a frontend generation for the board's
control identities. A held pedal or knob cannot edit the replacement project.
This resets the board view; meter latches keep a separate stable identity so
an acknowledged OVER count does not light again without another event.
It edits a copy of the project's `MasterChain`; `src/ui/view.rs` sends a changed
copy as `EditCommand::SetMaster` and passes the session's meter reading. The
preset selector offers the factory boards of `src/master/presets.rs`, which
the frontend reaches through `application::presets`; a chosen preset is one
`SetMaster` edit that keeps the board's trim, bypass, and keys (`Preset::applied_to`), and the
selector names the preset whose pedals the board's equal, whatever the trim or keys. Dial
values read in their units, the Booster EQ in decibels and the other Booster knobs in
the original's 0 to 1000 steps. Dials move on their knob's grid, and a fine
step still moves one; a typed value takes the nearest step. Displaying or
focusing a dial preserves saved values between steps. The OVER control exposes
its latched state as text to assistive technology.

`src/ui/step_synth.rs` owns Step synth's source, bar, grid, effects, and action
content inside the shared shell. Text, grid cells, switches, and input stay live.
The pitch grid and replacement effects form have bounded scrolling; source,
step labels, actions, and feedback remain reachable. The grid exposes sixteen
steps and twenty-five pitch rows. The shared `effects_controls` form retains its
algorithms and ranges and uses the common controls.

`src/ui.rs` owns the source path, viewed bar, effects-view selection, draft,
and local feedback. It adapts the panel's actions to existing `Session`
operations. An explicit WAV or PXD path overrides palette or clip selection,
including missing palette audio. Selecting it does not read audio.
`Session::step_synth_pcm` reads the source and renders synchronously when Listen
sequence or Save sequence is requested. The frontend then requests PCM preview
or the existing WAV save workflow. Save registers the result in Step synth at
the draft BPM and selects it. Save choice, cancellation, failure, and success
update local feedback. Missing palette audio offers Import details.
Pattern save/load uses `Session::save_pattern` and `Session::load_pattern`.
It replaces only the four-bar draft; the source path and viewed bar remain.
Clear bar clears only the viewed sixteen steps. The visible +12 through -12
pitch rows do not narrow the existing -48 through +48 serialized-note range.
This presentation does not add a worker.

Missing audio is explicit project state. The renderer leaves those placements
silent. The frontend reads the retained source metadata and sends a chosen WAV
path to `Session::resolve_missing_sample`. That operation resolves every matching
source identity in one history change, preserves musical trims, and retains the
chosen audio. The picker and context menu remain frontend responsibilities.

Duplicate file matches are stored in `Project.sample_conflicts`, with a stable
default, candidate paths, and source IDs. Readers select the first candidate.
`Session::resolve_sample_conflict` changes all affected placements atomically,
and updates history and rendering. The native modal exposes choices and playback;
confirmation and selected paths persist in the project. Transactions remove
conflicts that no longer affect any clip.

`src/main.rs` is the composition root. It resolves command-line options and builds
the initial library/project before it creates a session. Normal GUI startup then
scans the library folders through `Session`; a queued song opens only if no intervening
project change or unsaved take requires its cancellation. Successful folder
adoption tells the frontend to clear selections and active drags. Headless CLI
commands can use library services directly. The GUI modules are compiled into the native
binary, not exported by the production Rust library.

A scan lists up to 9 library folders, probes their WAVs with one progress
total, and merges them into one `Library`. A folder that cannot be listed, or
has no readable WAV, goes to `Library::unloaded_roots` and the others load; the
scan fails only when none loads. `sample-folder.json` keeps the ordered folder
list, and reads the single `root` that earlier versions wrote. A scan drops the
cache entries of files that its folders no longer list. Imported sample IDs
start at 1,000,000, above the 800,000 native IDs, and grow with the library.
Folder scans stream validated WAV samples into the catalog identity hash. Each
worker retains a reader buffer, not the decoded audio. File lists, metadata, and
the probe cache still grow with the corpus. `wav.rs` owns RIFF chunk traversal,
format validation, and buffered decoding for `audio::visit_wav_samples` and
the optional provider's WAV index. It supports mono/stereo PCM8/16/24/32,
float32, and their extensible subformats. Classic PCM24 in four-byte containers
retains its existing right-aligned decoding; extensible PCM uses its declared
container width with valid bits left-aligned. The reader validates the full
subformat GUID, extension length, frame size, byte rate, and required chunk
bounds. It discards a partial final audio frame but rejects a file missing a
whole frame. A scan writes the cache only after
new probes change it. Cached probes carry a decoder version; older probes are
decoded again so their audio identities agree with playback. `source_tempo.rs`
resolves folder tempo from matching
original audio before the duration-based fallback is used. A pack's `BPM.INF`
and tempo tokens in file names are stated tempos. They override the duration
reading but not matching original audio. Optional supplemental tempo identities
come from `source-tempos.json` in the runtime catalog folder.

## Runtime data folders and Album Review

Some application data lives beside the executable and is read at run time, not
compiled in. `src/runtime_data.rs` finds such a folder, `assets/<name>`, in the
executable's folder or up to four folders above it: the repository root for a
build in `target/release` or `target/debug/deps`, or the installation folder.

`src/catalog/releases.rs` reads the sample identity catalogs,
`assets/catalog/*.json`. Each file describes one release of one original
product (format 3): its product, a release name, the size and hash of each
original source file, a fingerprint of those files, and the identity of each
sample. A catalog holds no original names or paths: a sound's name is its
`label_key`, a one-way digest that still matches a WAV named after the sound.
`catalog::catalogs()` reads the folder on first use; `catalog::refresh()` reads
it again when a JSON file was added, changed or removed. `Session` calls it
before each sample folder scan. Before the window opens a song,
`Session::refresh_catalogs` calls it, and a changed folder rescans the sample
folders and opens the song after the scan. Each scan uses one catalog reading
for both native identities and authenticated source tempos. A change during a
scan restarts it with its current folders and fallback tempo. Cancellation or
failure keeps the rescan pending. A deferred open protects edits made after it
was queued, while accepting unsaved changes already approved for replacement.
Each reading stays valid for the rest
of the process, so identities already handed out never dangle. Releases of
one product merge into one identity list when they agree on audio and timing
for every native ID they share. The loader retains alternative label keys and
the strongest available audio digest. A release that gives a shared ID different
audio, tempo, or bar count, an invalid file, or
a second copy of a loaded release does not load and is listed in
`Catalogs::problems`, which startup reports. `project_io::open_song` adds a
source notice for each product whose catalogs lack some of a `.mix` song's
native IDs, unless an original installation serves that product.
The repository carries the generated catalogs in `assets/catalog`. A build or
installation without them cannot identify native WAVs from those catalogs and
retains unresolved sources as placeholders.

`src/album_review.rs` owns the Album Review: `Shelf` reads
`assets/album-review/<id>.json`, sorted by id, and lists each file that does
not load with its reason; a missing folder is an explicit state.
`album_review/review.rs` defines the saved form (`Review`, `Track`, `Event`,
`Energy`, `Kind`) and the implemented validation rules. `Shelf::add` and
`Shelf::update` check those rules before `atomic_file` writes the file. They do
not check the accuracy of factual or listening claims. New reviews use exclusive publication
and cannot replace an existing file, even when that file did not load.
`Query` and `Shelf::tracks` answer the agent
filters (artist, tag, energy, kind). `album_review/collection.rs` owns the
grid's rules: accent-insensitive search with `deunicode`, artist union, genre
intersection with spelling variants joined, the genre counts, and the five
sorts with their tie-breaks. `album_review/covers.rs` owns the cover cache in
the data folder (`album-review/covers/<id>.<ext>`, at most 256 MB, least
recently viewed first, with the view time kept as the file's modification
time) and, in the GUI build, the download: `ureq` with rustls, http and https
only, a 20-second timeout, at most 20 MB, and only a JPEG, PNG, or WebP image,
with the user agent `MixJamTechno/<version>`. `CoverLoader` runs reads,
downloads, and decoding on a background thread and the window polls it.
Downloaded images must decode before they enter the cache; an explicit
download can replace a corrupt cached image. Cache clearing waits until all
pending cover results have been consumed. Cover download is the
application's only network access.

`src/ui/album_review.rs` is the window. It keeps the filter, the sort, the
page, and the cover textures, and reaches the reviews only through
`application::album_review`. `agent::album_reply` answers the review requests
from a `Shelf`, so they work headless and without the window.

## Agent control

`gui --agent` lets a local program operate the window. The channel has two parts
on the two sides of the application boundary.

`src/agent.rs` is backend code without GUI dependencies. It defines the typed
requests and replies, encodes them as JSON lines, serves them on the loopback
interface, and contains the `agent` command-line client. It queues each request
for the frontend and wakes it. It does not interpret a request. Each session
has a new key in `agent-PORT.key` in the data folder. The key file uses the
file system's default permissions; the application sets no restrictive access
control list. Any user or program that can read the key can send
requests. A connection that does not start with the key, or that sends a
malformed line, is closed. An HTTP form post without the key does not reach the
queue. A request that the window does not take within 30 seconds fails and can
no longer run.
Dropping the server removes the session key, closes active connections, and
clears its window wake callback. It also wakes the listener and waits for it to
release the port. If the server cannot connect to wake the listener, it does not
wait, so the window can still close.

`src/ui/agent.rs` is the frontend driver. `eframe` calls it before each frame.
It reads the controls from the accessibility tree that egui produced for the
previous frame, and it acts through that frame's input: an accessibility action
for a named control, or pointer and keyboard events for a position. A request
replies after the frames that show its result. Arrangement requests call the
same `Session` operations as the lane gestures, because the lanes are one painted
surface without separate controls. The driver holds no project state and
reaches the backend only through `application.rs`; the source guard covers it.

egui does not mark the controls behind a modal as disabled, and an
accessibility action reaches them. Each modal therefore names itself with
`name_dialog`, and the driver refuses a control outside the open modal.
Typed arrangement commands obey the same boundary. Collapsing the dialog
releases those commands. Setting a radio button to true selects it; clearing
the selected button requires choosing another option.

Every file choice is a `FileRequest`. Without an agent, `ask_path` shows the
native dialog and carries the request out at once. With an agent, the request
waits in the Choose path modal, because an agent cannot operate a native dialog
and a blocked window cannot reply. A typed path gets the same result as a
chosen one, including the file extension that a save dialog adds.

A control is usable by an agent, and by a screen reader, only when it has an
accessible name. Give a new control visible label text, `widget_info`, or
`name_control`. Keep each name unique among the controls that can be on screen
together: the shared panel names its combo `Bottom module`, while the tool
rail has a `Mixer` shortcut. Sound-library filters keep their own names.
Add a control that is painted on a shared surface to the
arrangement requests instead.

## Safe frontend iteration

Change workspace layout in `src/ui/view.rs` and Mixer presentation in
`src/ui/mixer.rs`. Change floating shell geometry and lifecycle in
`src/ui/dialog.rs`, and shared control rendering and interaction in
`src/ui/controls.rs`. Tool bodies remain in `src/ui/stretch.rs`,
`src/ui/effects.rs`, `src/ui/step_synth.rs`, and `src/ui/sample_editor.rs`.
Keep tool action and file-dialog adapters in `src/ui.rs`. Reuse the same shell
and controls for every in-app window and modal; keep domain behavior in its caller.

Add a typed operation in
`application.rs` when a design needs new application behavior. Do not expose a
mutable project or add a callback that can change arbitrary application state.

Run the headless suite to check the application without the GUI feature. Run the
full suite to check native pointer and keyboard input, selection, scrolling,
and accessible actions. Source guards reject frontend imports and inline paths
that name an implementation module, including those in `src/ui/capture.rs` and
`src/ui_icons.rs`. They also reject selected mutation and I/O patterns
and public GUI exports. Private session state and read-only accessors enforce
state ownership at compile time. The headless build checks operation without
GUI dependencies. The source guard does not check the full dependency graph.

```powershell
.\tools\runtime\cargo.ps1 test --workspace --all-targets --no-default-features --locked
.\tools\runtime\cargo.ps1 test --workspace --all-targets --locked
.\tools\runtime\cargo.ps1 -CargoArguments @('clippy','--workspace','--all-targets','--locked','--','-D','warnings')
.\tools\runtime\cargo.ps1 -CargoArguments @('fmt','--all','--','--check')
python tools/check-rust-format.py
```

`tools/check-rust-format.py` reports Rust code that the format check passes but
rustfmt does not format: statements with a line too long to fit, and files that
reach the crate only through `include!`.

PowerShell removes a bare `--` from positional script arguments. Use the
`-CargoArguments` form for any command that passes arguments after `--`.

Run `python tools/assets/check-raster-assets.py` after asset or source changes.

`tools/verify-ui.ps1` checks the built native executable at 1440 by 1000,
1024 by 768, and 960 by 720. It creates synthetic WAV/project fixtures and uses
isolated settings. These captures need visual inspection. They do not test
physical audio, native file-picker interaction, high DPI, or screen readers.
Mixer capture cases include a full 64-track bank, an empty arrangement, and
attenuation/silence/unity gain, independent polarity, pan, and output-routing
states. Native egui tests check
control input and emitted accessibility semantics. They do not replace a screen
reader check of the built executable.

Syncopate Bold 700 sets the product name in every theme. Each theme adds a
heading face and a text face (see `docs/DESIGN.md`): IBM Plex Sans SemiBold
with Inter Medium for Studio, Michroma with Fira Sans Medium for Aero, and
Barlow Condensed SemiBold with Barlow Medium for Vintage. Inter is every
theme's symbol fallback. All files are unchanged from pinned sources. The
frontend transliterates branding and title letters to Latin counterparts and
formats them in uppercase without changing saved project names or paths.
Heading rows are pinned to one line height in every theme, so the layout does
not move. Window IDs remain independent of visible title formatting.
Run `python tools/assets/prepare-fonts.py --check` to verify font and license
hashes, static weights, families, embedding permission, ASCII coverage, and
Romanian coverage of text faces.
