# FX buses, master pedalboard, and export

## Contents

[Library replacement and song
opening](library-replacement.md#library-replacement-and-song-opening) has its
own page.

## FX buses

`src/fx.rs` owns the buses:

- `FxBuses` is the saved settings.
- `DelayParam`, `ReverbParam`, and `DistortionParam` name each continuous
  control with its range.
- `ReturnsProcessor` (`src/fx/processor.rs`) runs the buses on the feeds and
  adds their returns to the dry sum, before the master pedalboard.
- `src/fx/settings/` holds the settings of each effect, with their controls,
  ranges, and factory presets: `delay.rs`, `reverb.rs`, `drive.rs`, and the
  shared `range.rs`.

`Module::presets` gives the factory presets of an effect, each with the return
level that it sets. `FxBus::load_preset` sets each control but the duck key,
and the return level. `FxBus::matching_preset` gives the preset that the
settings and the return level of the bus agree with. The FX returns board
shows Custom when no preset agrees.

The processor sizes and allocates its delay lines at creation. Thus the audio
thread never allocates, also when a limiter resets.

The return lane of a bus (`Project::return_automation`, which the mix resolves
into `ReturnLanes`) replaces its return level frame by frame. The mix gives the
lanes and the song frame of the block to `ReturnsProcessor::process_at` and
`split`. The bus multiplies the output of its effect by the straight-line value
of the lane just before its safety stage. Live delay and reverb read the lane
`LOOKAHEAD` frames ahead. Live drive reads it at the current frame. Offline, the
processor reads the lane at the time of the effect sample before latency
compensation. A lane that moves after the end of the song shapes the tail. The
state of the effect runs independently of the lane, as it does behind a Return
knob at zero.

A bus with no input and no remaining internal tail sleeps. The delay sleep gives
a pending long echo enough time. Bypass does not hide internal tail activity
from this check. Clear tail and effect changes fade the return over 12 ms after
the safety stage, then clear the effect state. Returns run at
`RenderQuality::Live` during playback and `RenderQuality::Offline` during export
and stems:

- `RenderQuality::Live` keeps zero dry latency compensation (Emberform Drive
  uses 2x polyphase IIR oversampling and an instant soft safety ceiling, while
  Delay and Reverb use the 220-frame lookahead safety limiter with internal
  pre-delay/tap compensation; Echoform Delay uses 2x IIR saturation and cubic
  delay interpolation).
- `RenderQuality::Offline` runs Emberform Drive with 8x/16x linear-phase FIR
  oversampling and ADAA2, and Echoform Delay with 8x FIR ADAA2 input
  saturation and 16-tap Blackman-sinc read interpolation (`323` frames total).
  The safety limiter uses 4x true-peak lookahead with a -1.02 dBTP target when
  enabled. The dry path and master key tracks use the same delay
  (`ReturnsProcessor::latency_samples`, up to 324 frames).

The post-limiter ducker runs after the safety stage and aligns to delayed dry or
key transients within 2 ms. Live playback publishes per-bus peak, RMS, Ducker GR,
Limiter GR, and tail state through lock-free atomics in `MasterControl`. The
rack and editor share one display reading. Reduced mode holds the display for
at least 1/15 second and keeps the largest observed peaks and gain reduction
until the next update. The window shows idle readings when playback stops.
The monitoring solo mask starts clear for each song. An empty slot cannot hold
a solo.

Export, stem export, and playback share one block schedule
(`src/audio/schedule.rs`). After the song, it runs the stage on silence while a
bus rings, for at most `TAIL_LIMIT`, in 1024-frame steps (`Mix::TAIL_BLOCK`),
then drains offline FX and master delays. The fixtures in
`src/audio/tests/render_paths.rs` pin separate offline and live digests and
check live block-size invariance, RMS within 0.5 dB, and onset alignment
within two samples.

If an FX type change completes inside a processing block, its sidechain key
changes on that same frame. When the same inputs pass through blocks of
different sizes, the new key must not wait until the next block. A linked delay
uses the left tap time on both sides. Smear spreads feedback inside the loop,
and Resonance shapes the loop filters. The reverb's Bass and Treble controls
adjust decay in their bands. Each has a neutral value of 1. The reverb mixes
sixteen delay lines with a fast Hadamard transform. Offline renders read them
with a windowed sinc, and live playback uses cubic reads. The shimmer is a
stereo phase vocoder. Its hop grid depends only on the frames that passed, so
the block size does not change it, and a bus that slept wakes on the same grid.

A reverb delay change crossfades from a snapshot of all current delay
positions. Each read head limits its own modulation during the transition.
When the old positions stop their fade, the modulation returns to the
requested depth, within the bounds of the new space.

## Master pedalboard

`src/master.rs`, with its `src/master/` modules, owns the master pedalboard:

- `MasterChain` (`chain.rs`) is the saved settings.
- `Knob` (`knobs.rs`) names each continuous setting with its range.
- `MasterProcessor` (`processor.rs`) runs the board over the summed stereo mix
  in blocks, with state that continues from block to block.
- `booster` holds the DSP of the Booster pedals.
- `pedals` holds the mastering pedals of this application.
- `meters` holds the playback meters.
- `dsp` holds the shared parts:
  - the setting glide (`Smoothed`)
  - the biquad filters and a chain of them (`cascade`)
  - oversampling and true-peak interpolation
  - the lookahead limiter gain (`LookaheadGain`)

The limiters of the pedals and of the FX buses use the same `LookaheadGain`.

`MasterChain` is `Copy`. Its `order` is a fixed array that names each pedal one
time. Thus a settings copy never allocates on the audio thread, and the state of
the whole board has a fixed size. A pedal that is off passes the signal
unchanged. Each pedal keeps its own state when the order changes.

Master processors prepare the shared oversampling and true-peak filter tables
before playback reaches the audio thread. The callback must not initialize or
wait for those tables, at first playback or at later pedal switches. The
mastering playback quality option changes only Soft clip, Tube, Maximizer, and
Tape. Their saturation uses second-order antiderivative antialiasing. Playback
uses a 4x FIR oversampler, or runs at the base rate with the option set to Off.
Both playback paths have a 69-frame stage delay. Export uses an 8x
linear-phase FIR oversampler with a 71-frame stage delay. The Maximizer
uses a shorter 4x filter during playback to fit its output peak guard within
the same stage delay. Its export delay is 107 frames, including the guard.
The option does not change these items:

- the limiter lookahead
- the true-peak interpolation
- the metering
- the other processors
- the fixed live delay

The mastering pedals glide to new settings over about 20 ms. During playback, if
you switch or move a pedal, the application copies the board. Playback then
crossfades from the old copy to the new copy over 30 ms. Live playback pads the
two paths to a fixed 593-frame delay. It fills the new path before the fade
starts. Thus a pedal switch keeps the timeline alignment of the pedals and the
end of the song.

`MasterProcessor` applies the trim, then holds the mix to full scale, and then
runs the pedals. Thus a lower trim prevents clipping of a hot mix. The mix and
the FX returns reach it without a clamp. Playback without a board holds the mix
to full scale. The `Mix::read` test helper also holds it to full scale.

`MasterChain::bypass` skips the trim and each pedal. `MasterChain::runs` is
false for each pedal, the trim glides to unity, and the board latency is zero.
Thus export has no pedal delay. During playback, the bypassed board still pads
to the fixed delay and still feeds the meters. A bypass switch crossfades as a
pedal switch does. When the board leaves bypass, the pedals that are on start
from a clear state.

`MasterChain::latency` is the sum of active offline pedal delays, in frames.
`MasterChain::latency_ms` gives the export delay in milliseconds. `Mix::to_pcm`
and `Mix::write_wav` run the pedals past the end by that delay and cut it from
the start. Playback drops its fixed delay before the first sample and drains the
same number of frames at the end. Thus the two stay on the timeline. Export and
`Mix::to_pcm` take the chain of the project explicitly. `Mix::to_pcm` holds the
whole result and has no length limit. It reserves the memory first, and reports
a plain error if the memory is not available. `Mix::write_wav` and
`Mix::write_stems` render in blocks. Their length limit is `Mix::MAX_WAV_FRAMES`
(about 6.7 hours). It fits the 32-bit WAV size fields, with 36 bytes of RIFF
header. Both check it before they open a destination file.

Mix playback reads a `MasterControl` that `Session` keeps equal to the project.
The audio thread takes new settings at its next block through the lock-free
`audio::Latest` cell, so it never waits. An edit or history step that changes
only the pedalboard or the FX buses keeps the prepared mix and the playback.
This holds when the set of necessary key tracks stays the same. A changed set of
keys prepares the mix again. The same `MasterControl` carries the settings of
the FX buses and the Clear Tail requests. The `ReturnsProcessor` polls them in
the same way. Tool previews play without the pedalboard. Each playback and
export starts the pedalboard in the same settled state.

### Meters

A live processor also measures. Each 1024 frames, it publishes a `MeterReading`
through atomics in the `MasterControl`, and `Session::master_meters` reads it
without a wait. Each playback gives a number to its readings, and only the start
of a playback changes that number. A stopped playback that is still in its last
block can show its levels for one block. But the readings of the next playback
still reach the window. The cumulative OVER count carries its playback number in
the same atomic value. Thus a stale count cannot become the lamp baseline of the
new playback.

Readings change more than 30 times a second. Each reading covers the same frames
for each playback block size. Thus the block size does not change the true peak
shown or the OVER count. At the end, playback publishes the frames since the
last reading and finishes the peak detector. It adds no audio frames and no
loudness time. The output meters skip the initial 593 frames that playback
removes, so the loudness windows follow the audible program. Export does not
measure.

The moving minimum of the Multiband keeps exactly the active lookahead window,
also when the gain requirements increase for longer than its fixed queue. It
removes old entries before it adds a new entry, so a full queue cannot overwrite
its live minimum.

## Stems, render worker, and export

`Mix::write_stems` exports the premaster in parts, in one pass over the song.
Each sounding track mixes alone through its own `RenderState`, and its sends add
into the feeds of the buses. `ReturnsProcessor::split` gives the return of each
bus separately, without a clamp. The parts follow the block schedule of
`render`, without the delay of the master pedals. The render state of the first
track measures every key and keeps the key audio for the returns. The other
render states measure only the key of their own ducker. Thus each file has the
same length as the mix export, and the parts skip the master pedalboard. PCM16
stem export rejects a non-finite or out-of-range part before it publishes files,
instead of clipping each part separately. The sum of the parts equals the
bypassed mix when that mix does not clip. `atomic_file::write_many` stages each
file before it replaces a file.

The render worker prepares the mix. It keeps the decoded sources of the last
mix, and discards them when the application replaces the library. With audio
output, `Session` asks the worker for each new revision when a song opens or the
application accepts a render edit. Thus the mix is ready when the user selects
Play. If the preparation of a revision fails, the revision waits for Play, which
asks again and reports the error.

`Session::start_export` renders and writes on its own thread and reports
through `poll`. `Session::export` stays synchronous for headless callers, such
as the `render` command. `Session::start_track_export` and
`Session::export_tracks` do the same for the parts. `export_tracks` makes the
folder and returns the number of track and return parts (`audio::Stems`). Only
one export of either type runs at a time.
