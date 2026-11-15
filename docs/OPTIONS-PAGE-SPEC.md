# Options page specification

Status: implemented. The owner accepted the recommended baseline in
[Open decisions](#open-decisions). The options do not change the song format.
[OPTIONS-VERIFICATION.md](OPTIONS-VERIFICATION.md) records the verification
results and the manual checks that are not done. This status does not
establish release readiness or a supported hardware limit.

## Purpose and boundaries

The Options window gives the user one place to select playback quality and
application preferences. The defaults keep the full sound quality. A lower CPU
mode is available for a project that does not play reliably on the user's
computer.

Options is a native window with four pages: Audio, Workspace, Files, and Sample
libraries. It uses the shared Utility shell and controls in the current theme.
It is not a web page, a replacement for the Mixer, or a new song editor.

In scope: the controls in [Options pages](options-spec/pages.md), preference
persistence, safe application of changes, synchronization with existing
controls, agent access, and verification.

Out of scope:

- Device selection, ASIO or exclusive audio, and sample-rate changes.
- Plugins and autosave.
- Theme editing, custom themes, and arbitrary UI scaling.
- New song DSP parameters and changes to the song format.

Do not show an internal processing block size as a device latency control.
Options use their own schema, separate from the song format.

## Implementation owners

| Area | Behavior and owner |
|---|---|
| Track Drive | Four-times oversampling in `src/drive.rs`. The shared `Oversampler` in `src/master/dsp.rs` has a 69-frame delay. `TrackChain` in `src/audio/track.rs` compensates that delay and bounds reads at playback loop ends. |
| Master saturation | Soft clip, Tube, Maximizer, and Tape use `Oversampler` through `src/master/pedals.rs`. The other master processors have their own algorithms and timing. |
| Master timing | `MasterProcessor` in `src/master/processor.rs` keeps a fixed live delay across pedal changes. Export compensates the active delays. |
| Playback work | `PlaybackSource` in `src/audio/source.rs` renders 256-frame blocks. This is not the callback size of the audio device. |
| Output buffering | `src/audio/output.rs` requests the selected output buffer from CPAL, bounded by the limits that the device reports, with a system-default fallback. The request and the callback interval are separate values. |
| Preferences | `src/options.rs` defines `AppOptions` and its file. `Session` validates, applies, and persists the active values. Its options operations are in `src/application/settings.rs`. |
| Native controls | `src/ui/options.rs` owns the window and its draft. `src/ui/dialog.rs` owns the window shells, and `src/ui/controls.rs` owns the shared controls. The application API of the frontend is `Session`. |

Oversampling costs CPU and adds a signal delay. The application compensates
that delay. Off decreases CPU work. The page must not promise lower device
latency. [Options verification](OPTIONS-VERIFICATION.md) holds the
measurements of the buffer targets and the CPU cost.

## Options

The contract for each page is in [Options pages](options-spec/pages.md).
The interaction rules, the DSP implementation contract, and the preference
ownership and persistence are in [Options rules](options-spec/rules.md).

## Agent and accessibility contract

- Expose the window and each control through the existing named UI channel.
- Use stable names that match the option labels, with the selected values,
  units, disabled reasons, and keyboard operation.
- Add the active preference values and an audio-apply-blocked reason to agent
  `state`, separate from the song state.
- Agent Apply must use the same validation, stop requirements, history
  exclusion, and error path as a click.
- A direct change to the JSON file is not a replacement for the UI.

Test keyboard-only page navigation, value editing, Apply, Cancel, Restore
defaults, and focus restoration. Inspect the accessibility tree. Test with a
screen reader when one is available. A screenshot alone does not prove
accessibility.

## Verification and acceptance

Do not enable a new quality control before its processor tests and its
export-isolation tests pass. When a behavior changes, update the product,
architecture, design, user-guide, agent-control, and development contracts.

Run the current commands in [DEVELOPMENT.md](DEVELOPMENT.md): both Cargo
feature sets, Clippy with warnings denied, both format checks, the Python
tests, the release build, the applicable asset checks, and the native
captures. Follow [ARCHITECTURE.md](ARCHITECTURE.md), [DESIGN.md](DESIGN.md),
and [AGENT-CONTROL.md](AGENT-CONTROL.md). Keep the public raster-asset rule.

For each quality combination, test the preparation and the active processing
of these cases:

- Drives that are off, zero Mix, and maximum Drive.
- The four master saturation pedals alone and together.
- Sidechain keys.
- Short loops, discontinuous seeks, and source gaps.
- Stem summation.
- A preference change while an old job completes.

Make sure that a sample-rate conversion by the device cannot change the
selected internal quality or the musical timing. Run the saved full-quality
artifact fixtures with different preference files, and compare the output
samples and the duration.

Measure an optimized release with the callback harness in
`src/audio/realtime_bench.rs`:

- Include a representative arrangement, sparse and dense Drive use, a
  master-heavy case, and a 64-track stress case.
- Measure starts, seeks, replacements, and continuous playback after all
  relevant lanes have played.
- Report the CPU and OS, the device or callback model, the output rate, the
  workload, the duration, the mean, p99, maximum, and missed deadlines.
- Do the measurement again on a quiet machine to separate sustained overload
  from single scheduling spikes.

Off must have a lower measured CPU cost in a DSP-bound case. Do not claim that
Off makes each project fit a 10 ms deadline. Before you claim release
readiness for a hardware capacity, choose the minimum supported test machine
with the owner.

Listen to the Off-mode tradeoff on bass, bright tones, drums, and dense mixes.
Check clicks, delay alignment, seeks, loop boundaries, and physical playback at
44.1 and 48 kHz. Record the microphone, native-picker, screen-reader, and
clean-machine checks separately as passed, failed, or not available.

## Open decisions

The accepted baseline:

- Separate Drive and master playback switches, with 4x defaults.
- Exports always at 4x.
- Audio changes apply only while playback, previews, and recording are
  stopped.
- A supported output buffer target, with truthful reports of the actual
  period.
- No device picker.

These items are product decisions outside the baseline:

- Draft exports.
- Live quality switching.
- The minimum supported performance machine.

Before you expand the baseline, get a decision from the owner on each item.
Do not infer approval from this specification.
