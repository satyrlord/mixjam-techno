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

The processor sizes and allocates its delay lines when it is built. Thus the
audio thread never allocates, also when a limiter resets.

The return lane of a bus (`Project::return_automation`, which the mix resolves
into `ReturnLanes`) replaces its return level frame by frame. The mix gives the
lanes and the song frame of the block to `ReturnsProcessor::process_at` and
`split`. The bus multiplies the output of its effect by the straight-line value
of the lane just before its safety stage. It reads the lane as many frames
ahead as the return of the bus is late: `LOOKAHEAD` for a delay or a reverb,
and none for a drive. Thus each returned frame carries the value of the lane at
its own time. The song frame continues to count through the tail, in export
and in playback. Thus a lane that moves after the end of the song shapes the
tail in the same way. The lane only scales the return. The state of the effect
runs independently of the lane, as it does behind a Return knob at zero.

A bus with no input and no remaining internal tail sleeps. The delay sleep
gives sufficient time for a pending long echo. Bypass does not hide internal
tail activity from this check. Clear tail and effect changes fade the return
over 12 ms after the safety stage, then clear the effect state. The safety
limiter has a fixed 220-frame lookahead. The delay takes that lookahead off its
output tap, but its feedback
keeps the full selected time, so each repeat stays on the beat. The reverb
compensates its pre-delay where possible.

The Emberform Drive (`src/fx/distortion.rs`) has no delay to make shorter, and
its return adds to the dry sound that it came from. Thus its bus adds no
latency: an instant ceiling (`ceiling` in `src/fx/parts.rs`) replaces the
limiter and its lookahead.

Export, stem export, and playback share one block schedule
(`src/audio/schedule.rs`). After the song, it runs the stage on silence while a
bus rings, for a maximum of `TAIL_LIMIT`, in 1024-frame steps
(`Mix::TAIL_BLOCK`). It asks once at the start of each step whether a bus still
rings, independently of the size of the caller's blocks. Then it runs the delay
of the master pedals for the mix export and playback. The
[playback contract](playback.md#output-device-and-the-audio-thread) describes
when the samples match. A digest test (`src/audio/tests/render_paths.rs`)
checks the samples of each render path.

If an FX type change completes inside a processing block, its sidechain key
changes on that same frame. When the same inputs go through in different block
sizes, the new key must not wait until the next block. A reverb delay change
crossfades from a snapshot of all current delay positions. Each read head
limits its own modulation during the transition. When the old positions stop
their fade, the modulation goes back to the requested depth, within the bounds
of the new space.

## Master pedalboard

`src/master.rs`, with its `src/master/` modules, owns the master pedalboard:

- `MasterChain` (`chain.rs`) is the saved settings.
- `Knob` (`knobs.rs`) names each continuous setting with its range.
- `MasterProcessor` (`processor.rs`) runs the board over the summed stereo mix
  in blocks, with state that continues from block to block.
- The other submodules hold the DSP of the Booster pedals (`booster`), the
  mastering pedals of this application (`pedals`), their shared parts
  (`dsp`), and the playback meters (`meters`).
- The shared parts are the setting glide (`Smoothed`), the biquad filters and
  a chain of them (`cascade`), oversampling, true-peak interpolation, and the
  lookahead limiter gain (`LookaheadGain`). The limiters of the pedals and
  of the FX buses use the same `LookaheadGain`.

`MasterChain` is `Copy`, and its `order` is a fixed array that names each
pedal one time. Thus a settings copy never allocates on the audio thread, and
the state of the whole board has a fixed size. A pedal that is off passes the
signal on unchanged. Each pedal keeps its own state when the order changes.

Master processors prepare the shared oversampling and true-peak filter tables
before playback gets to the audio thread. The callback must not initialize or
wait for those tables, at first playback or at later pedal switches. The
mastering playback quality option changes only Soft clip, Tube, Maximizer, and
Tape. When it is Off, their saturation runs at the base rate, with the same
integer stage delay. It does not change the limiter lookahead, the true-peak
interpolation, the metering, the other processors, or the fixed live delay.

The mastering pedals glide to new settings over about 20 ms. During playback,
when a pedal is switched or moved, the board is copied, and playback
crossfades from the old copy to the new copy over 30 ms. Live playback pads
the two paths to a fixed 593-frame delay. It fills the new path before the
fade starts. Thus a pedal switch keeps the timeline alignment of the pedals and
the end of the song.

`MasterProcessor` applies the trim, then holds the mix to full scale, and then
runs the pedals. Thus a lower trim prevents clipping of a hot mix. The mix and
the FX returns get to it without a clamp. A playback without a board, or
`Frames::read`, holds them instead.

`MasterChain::bypass` skips the trim and each pedal. `MasterChain::runs` is
false for each pedal, the trim glides to unity, and the board latency is zero.
Thus export has no pedal delay. During playback, the bypassed board still pads
to the fixed delay and meters, and a bypass switch crossfades as a pedal switch
does. When the board leaves bypass, the pedals that are on start from a clear
state.

`MasterChain::latency` is the delay of the pedals that run, in frames.
`MasterChain::latency_ms` gives it in milliseconds for the board.
`Mix::to_pcm` and `Mix::write_wav` run the pedals past the end by that delay
and cut it from the start. Playback drops its fixed delay before the first
sample and drains the same number of frames at the end. Thus the two stay on
the timeline. Export
and `Mix::to_pcm` take the chain of the project explicitly. `Mix::to_pcm` holds
the whole result and has no length limit. It reserves the memory first, and
reports a plain error if the memory is not available. `Mix::write_wav` and
`Mix::write_stems` render in blocks. Their length limit is
`Mix::MAX_WAV_FRAMES` (about 6.7 hours), which fits the WAV size field.

Mix playback reads a `MasterControl` that `Session` keeps equal to the
project. The audio thread takes new settings at its next block through the
lock-free `audio::Latest` cell, so it never waits. An edit or history step that
changes only the pedalboard or the FX buses keeps the prepared mix and the
playback, when the set of necessary key tracks stays the same. A changed set of
keys prepares the mix again. The same `MasterControl` carries the settings of
the FX buses and the Clear Tail requests, which the `ReturnsProcessor` polls in
the same way. Tool previews play without the pedalboard. Each playback and
export starts the pedalboard in the same settled state.

### Meters

A live processor also measures. Each 1024 frames, it publishes a
`MeterReading` through atomics in the `MasterControl`, and
`Session::master_meters` reads it without a wait. Each playback gives a number
to its readings, and only the start of a playback changes that number. A
stopped playback that is still in its last block can show its levels for one
block. But it cannot stop the readings of the next playback from getting to
the window. The cumulative OVER count carries its playback number in the same
atomic value. Thus a stale count cannot become the lamp baseline of the new
playback.

Readings change more than 30 times a second. Each reading covers the same
frames for each playback block size. Thus the block size does not change the
true peak shown or the OVER count. At the end, playback publishes the frames
since the last reading and finishes the peak detector, without more audio
frames or loudness time. Output metering does not include the initial 593
frames that playback removes, so the loudness windows follow the audible
program. Export does not measure.

Stereo PCM16 export reserves 36 bytes of the RIFF size field for its header
before it opens a destination file. The moving minimum of the Multiband keeps
exactly the active lookahead window, also when the gain requirements increase
for longer than its fixed queue. It removes old entries before it adds a new
entry, so a full queue cannot overwrite its live minimum.

## Stems, render worker, and export

`Mix::write_stems` exports the premaster in parts, in one pass over the song.
Each sounding track mixes alone through its own `RenderState`, and its sends
add into the feeds of the buses. `ReturnsProcessor::split` gives the return of
each bus separately, without a clamp. The parts follow the block schedule of
`render`, without the delay of the master pedals. The render state of the first
track measures every key and keeps the key audio for the returns. The other
render states measure only the key of their own ducker. Thus each file has the
same length as the mix export, and the parts skip the master pedalboard. PCM16
stem export rejects a non-finite or out-of-range part before it publishes
files, instead of clipping each part separately. The parts add up to the
bypassed mix when that mix does not clip. `atomic_file::write_many` stages each
file before it replaces a file.

The render worker prepares the mix. It keeps the decoded sources of the last
mix, and discards them when the library is replaced. With audio output,
`Session` asks the worker for each new revision when a song opens or a render
edit is accepted. Thus the mix is ready when the user selects Play. If the
preparation of a revision fails, the revision waits for Play, which asks again
and reports the error.

`Session::start_export` renders and writes on its own thread and reports
through `poll`. `Session::export` stays synchronous for headless callers, such
as the `render` command. `Session::start_track_export` and
`Session::export_tracks` do the same for the parts. `export_tracks` makes the
folder and returns the number of track and return parts (`audio::Stems`). Only
one export of either type runs at a time.
