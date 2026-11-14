# FX buses, master pedalboard, and export

## FX buses

`src/fx.rs` owns the buses:
`FxBuses` is the saved settings, `DelayParam` and `ReverbParam` name each
continuous control with its range, and `ReturnsProcessor` (`src/fx/processor.rs`) runs the buses on
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
Reverb delay changes crossfade from a snapshot of all current delay positions.
Each read head bounds its own modulation during the transition. Once the old
positions finish fading, modulation returns to the requested depth within the
new space's bounds.

## Master pedalboard

`src/master.rs`, with its `src/master/` modules, owns the master pedalboard:
`MasterChain` (`chain.rs`) is the saved settings, `Knob` (`knobs.rs`) names
each continuous setting with its range, and `MasterProcessor` (`processor.rs`)
runs the board over the summed stereo mix in blocks, with state that carries
from block to block. Its submodules also hold the Booster pedals' DSP
(`booster`), this application's mastering pedals (`pedals`), their shared
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
explicitly. `Mix::to_pcm` holds the whole result and has no length limit: it
reserves the room first and reports a plain error if the memory is not there.
`Mix::write_wav` and `Mix::write_stems` render in blocks and refuse only a
song that is too long for a WAV file (`Mix::MAX_WAV_FRAMES`, about 6.7 hours). Mix playback reads a `MasterControl` that the
`Session` keeps equal to the project; the audio thread takes new settings at
its next block through the lock-free `audio::Latest` cell, so it never waits.
An edit or history step that
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

## Stems, render worker, and export

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

## Library replacement and song opening

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
