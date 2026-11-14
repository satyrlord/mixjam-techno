# Options page specification

Status: implemented, 2026-11-02. The owner requested
implementation of this specification on that date, including its recommended
baseline. The implementation adds the options below without changing the song
format. Verification results and manual-check limits must accompany delivery;
this status does not establish release readiness or a supported hardware limit.
See [OPTIONS-VERIFICATION.md](OPTIONS-VERIFICATION.md) for results and manual limits.

## Purpose and boundaries

Give the user one place to select playback quality and app preferences.
Keep the current sound and behavior as the default. Offer a lower CPU mode
when a project cannot play reliably on the user's computer.

Use a native Options window containing Audio, Workspace, Files, and Sample libraries pages.
Use the shared Utility shell and controls in the current theme. This is not a web
page, a replacement for the Mixer, or a new song editor.

In scope: the controls below, preference persistence, safe application of
changes, existing-control synchronization, agent access, and verification.
Out of scope: device selection, ASIO or exclusive audio, sample-rate changes,
plugins, autosave, theme editing or custom themes, arbitrary UI
scaling, new song DSP parameters, and a change to song format 24. Do not expose
an internal processing block size as a device latency control. The current
song format is 24; options use their own schema.

## Current evidence

Revalidate these facts before implementation; they describe the source at the
time of this proposal.

| Area | Current behavior and owner |
|---|---|
| Track Drive | Four-times oversampling in `src/drive.rs`; the shared oversampler has a 69-frame delay. `TrackChain` in `src/audio.rs` compensates that delay and bounds reads at playback loop ends. |
| Master saturation | Soft clip, Tube, Maximizer, and Tape use `Oversampler` in `src/master/pedals.rs`. Other master processors have their own algorithms and timing. |
| Master timing | `MasterProcessor` in `src/master.rs` keeps a fixed live delay across pedal changes. Export compensates active delays. |
| Playback work | `PlaybackSource` renders 256-frame blocks. This is not the audio device's callback size. |
| Output buffering | `src/audio/output.rs` requests a 40 ms CPAL output buffer, bounded by reported device limits, with system-default fallback. The request and callback interval are separate values. Revalidate the device probe before choosing the page's defaults. |
| Preferences | `src/project_io.rs` stores sample folders, the last song folder, and section bookmark visibility. `Session` exposes their application operations. Follow currently starts enabled and lasts for the current session. |
| Native controls | `src/ui/dialog.rs` owns window shells; `src/ui/controls.rs` owns shared controls. The frontend's application API is `Session`. |

The review measured a 22-Drive workload on Windows 11 with an Intel i7-11700.
For 48 kHz output with 10 ms callbacks, removing duplicate work reduced mean
callback time from 13.52 to 8.49 ms; the optimized case still exceeded the
deadline in 45.3% of 1,000 measured callbacks. These are workload-specific
measurements, not a supported track-count limit or a minimum hardware claim.
The measurement started at a song position, so it does not establish the cost
of uninterrupted playback from the song's start. Keep private fixture paths
out of public documentation.

Oversampling costs CPU and also introduces a signal delay. The existing delay
is compensated. Turning oversampling off is intended to reduce CPU work;
the page must not promise lower device latency.

A silent probe on the default Steinberg UR-RT2 at 48 kHz observed 1,056
initial writable frames with system-default buffering and 1,920 with the
40 ms request. Both runs averaged about 10.1 ms between callbacks. This
confirms a change in the initial writable buffer on that device, not a
40 ms callback interval or glitch-free heavy-song playback. The backend
request is already implemented; this proposal adds user control over it.

## Options

The page-by-page contract is in [Options pages](options-spec/pages.md).
The interaction rules, the DSP implementation contract, and preference ownership and persistence are in
[Options rules](options-spec/rules.md).

## Agent and accessibility contract

Expose the window and each control through the existing named UI channel.
Use stable names matching the option labels, with selected values, units,
disabled reasons, and keyboard operation. Add active preference values and
an audio-apply-blocked reason to agent `state`, separate from song state.
Agent Apply must use the same validation, stop requirements, history exclusion,
and error path as a click. No direct JSON-file manipulation is a UI substitute.

Test keyboard-only page navigation, value editing, Apply, Cancel, Restore
defaults, and focus restoration. Inspect the accessibility tree and test with
a screen reader when available; a screenshot alone does not prove accessibility.

## Delivery slices and exit checks

| Slice | Dependency | Deliverable and observable exit check |
|---|---|---|
| 1. Preference model | None | Typed defaults, validation, atomic persistence, existing section-visibility migration, and Session API. Tests prove corrupt/future files are preserved and failed Apply changes nothing. No new UI or DSP behavior yet. |
| 2. Native shell and app options | 1 | Options navigation, draft lifecycle, Workspace/Files controls, preview level, and meter display cadence. Tests prove project/history independence, toolbar synchronization, stale-draft rejection, and preview/export separation. Capture all three window sizes. |
| 3. Drive playback quality | 1-2 | 4x/Off for audible and key chains. 4x matches the current reference; Off keeps placement/timing/zero-Mix invariants. Full-quality exports are identical regardless of playback preference. Measure callback costs before and after. |
| 4. Output buffering | 1-2 | Extend the existing 40 ms backend request with supported targets, stopped stream replacement, truthful request/cadence reporting, and failure rollback. Record device acceptance and actual callback behavior before making buffering or performance claims. |
| 5. Master playback quality | 3 | Separate quality for the four named saturation pedals. Verify parallel dry paths, live delay, pedal edits, key timing, and limiter/true-peak behavior. The preference does not disable unrelated processing. |
| 6. Integration and handoff | 2-5 | Agent state/control coverage, documentation, release-artifact verification, performance evidence, and stated manual-check limits. No placeholders, disabled future controls, or temporary migration writes remain. |

Each slice must remain independently reviewable. Do not expose an enabled
quality control before its processor and export-isolation tests pass. The
implementing agent should update the current product, architecture, design,
user-guide, agent-control, and development contracts as behavior lands.

## Verification and acceptance

Run the current commands in [DEVELOPMENT.md](DEVELOPMENT.md), including both
Cargo feature sets, warnings-denied Clippy, both format checks, Python tests,
release build, applicable asset checks, and native captures. Follow
[ARCHITECTURE.md](ARCHITECTURE.md), [DESIGN.md](DESIGN.md), and
[AGENT-CONTROL.md](AGENT-CONTROL.md). Preserve the public raster-asset rule.

For each quality combination, test both preparation and active processing:
off Drives, zero Mix, maximal Drive, the four master saturation pedals alone
and together, sidechain keys, short loops, discontinuous seeks, source gaps,
stem summation, and a changed preference while an old job completes. Check
that a sample-rate conversion by the device cannot change the chosen internal
quality or musical timing. Run the saved full-quality artifact fixtures under
different preference files and compare output samples and duration.

Measure an optimized release with the callback harness in
`src/audio/realtime_bench.rs`. Include a representative arrangement, sparse
and dense Drive use, a master-heavy case, and 64-track stress. Measure starts,
seeks, replacements, and uninterrupted playback after all relevant lanes have
sounded. Report CPU/OS, device or callback model, output rate, workload,
duration, mean, p99, maximum, and missed deadlines. Repeat on a quiet machine
to distinguish sustained overload from isolated scheduling spikes. Require
lower measured CPU cost for Off in a DSP-bound case; do not claim it makes
every project fit a 10 ms deadline. Choose the minimum supported test machine
with the owner before making release-readiness claims about hardware capacity.

Listen for the Off-mode tradeoff on bass, bright tones, drums, and dense mixes.
Check clicks, delay alignment, seeks, loop boundaries, and physical playback
at 44.1 and 48 kHz. Record microphone, native-picker, screen-reader, high-DPI,
and clean-machine checks separately as passed, failed, or unavailable.

## Decisions for the owner or assigned agent

Recommended baseline: separate Drive/master playback switches; 4x defaults;
exports always 4x; audio changes apply only while stopped; a supported output
buffer target with truthful actual-period reporting; no device picker in this
version; the complete option list above delivered in slices.

Before expanding this baseline, resolve whether draft exports or live quality
switching are actually needed, and identify the minimum supported performance
machine. Those are additional product decisions. The assigned agent must not
infer approval for them from the existence of this proposal.
