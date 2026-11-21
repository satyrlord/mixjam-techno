# Review remedies

Use the owning contract to select a remedy. These examples do not add product
requirements.

- Move project edits, history, file operations, and audio control behind the
  `Session` API. Keep presentation and input gestures in the native UI.
- Send project edits through typed operations. Keep undo behavior. If the
  current contract requires one history step for a continuous gesture, group it.
- Validate projects and imports before adoption. On a failure, keep the active
  project. Keep save replacement atomic.
- Keep the migrations of unversioned JSON projects and of JSON schemas 1 to 3
  compatible with `PROJECT_SCHEMA_VERSION`.
- Keep the `.mixjam` reader compatible with each version that
  [SONG-FORMAT.md](../../../docs/SONG-FORMAT.md) says the reader opens.
- Check native sample identity with audio digests when they are available.
  Reject a digest mismatch. A name and duration match is only an inference,
  for use when no digest comparison is available.
- Cancel or discard stale background results before they change the active
  library, render, transport, or recording state.
- Test the affected render timing, channel routing, and audio format with
  deterministic files. Keep listening and device results separate.
- If a UI change affects native focus, accessible names, or compact layouts,
  check them in the built application.
- Keep original binaries, catalogs, media, snapshots, manifests, and Git
  bundles intact. Use them as evidence for parity, not as donor feature plans.
