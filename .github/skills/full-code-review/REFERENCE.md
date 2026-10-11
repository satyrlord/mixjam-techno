# Review remedies

Read this reference only after you confirm a defect. Use the owning contract to
choose a remedy. These examples do not add product requirements.

- Move project edits, history, file operations, and audio control behind the
  `Session` API. Keep presentation and input gestures in the native UI.
- Route project edits through typed operations. Preserve undo behavior and
  group continuous gestures when the current contract requires one history step.
- Validate projects and imports before adoption. Preserve the active project on
  failure and keep save replacement atomic.
- Keep schema 1 and unversioned project migrations compatible with schema 2.
- Check native sample identity with audio digests when available. Reject an
  available digest mismatch. Treat name and duration matching as inference
  only when a digest comparison is unavailable.
- Cancel or discard stale background results before they change the active
  library, render, transport, or recording state.
- Test affected render timing, channel routing, and audio format with
  deterministic files. Keep listening and device results separate.
- Check native focus, accessible names, and compact layouts in the built
  application when a UI change affects them.
- Keep original binaries, catalogs, media, snapshots, manifests, and Git
  bundles intact. Use them as evidence for parity, not as donor feature plans.
