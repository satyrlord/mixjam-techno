# Library replacement and song opening

`Session::catalog_revision` advances when the application replaces the library.
Until then, the frontend keeps its filtered palette. Folder adoption takes the
providers that are already loaded from the current library. It reads a provider
from disk only when the folder of that provider is not there.

When a saved provider replaces a partial product during song opening, it keeps
the loaded sample identities that the saved provider does not have. If an
original provider is already loaded, the opening also examines the saved WAV
folder of the song. It searches for used native IDs that the provider does not
have. It adds only those IDs, and it keeps the identities of the current
provider and the saved binding.

Sometimes the used native WAVs of one product cannot all be under one saved
folder. Then `project_io::save_with_library` keeps their exact file bytes
together under `native-providers/<product>/<content hash>` in the data folder.
Before it saves that binding, it verifies the native IDs, the decoded audio, and
the timing. Repeated saves use the verified content again. If this step fails,
the current song does not change. This keeps the native identities without a
change to the song format.

WAV path resolution includes verified duplicate copies that the palette does not
show. MixJam v7 suffix relocation and native WAV lookup use this complete path
inventory. Thus a reference to a duplicate still resolves after its folder
moves. The current song format records one provider for each product. That
binding cannot hold a song that uses a packed original provider and supplemental
WAVs for the same product. Save rejects this combination before it replaces the
song.

A song that waits for a folder scan records the render revision and the editor
generation. A project edit in that time cancels the deferred open. This includes
a master-only edit followed by Save. Master-only edits still keep the prepared
dry mix. When a project opens, it adopts freshly measured WAV metadata. This
includes changes to the length and channel count at the same path. History
changes can still use the current metadata again, without a new read of each
source.

The agent Choose path request for Open also records this edit identity. Assume
the user edits the song, or makes a recorded take, while that dialog is in its
collapsed state. Then a confirmation of the old request keeps the current song
and the path draft. The user must cancel and start Open again. Changes that the
user accepted before the request do not prevent the request from opening the
selected song.

While recording, or while a sample folder scan runs, the user cannot add or
remove an optional sample provider. The rejection keeps the current libraries,
the scan, the recorded take, the project, and the settings.
