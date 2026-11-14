# Sound tools, Album Review, and saving

## Sound tools and recording

- Use Import for WAV files and enter the known source tempo.
- Use Sample Editor to trim and change pitch. Start frame is included; End frame
  is excluded. The adjacent buttons change one frame or 0.1 semitone; hold Shift
  for 100 frames or one semitone. Click a value for exact entry. Listen edit
  previews the draft; Save edited sound writes a new WAV in Wave.
- Use the other sample tools to stretch tempo or apply effects, then save a new
  WAV sample.
- Use Step synth, a board of the lower panel, to sequence one note per
  sixteenth step across up to four bars. Save sequence writes the WAV and,
  beside it, its pattern as `Name.pattern.json`, which Load pattern also
  opens; the arrangement draws the sound's clips as notes from it. If the
  pattern cannot be saved, the tool reports the error and preserves existing files.
- Select a recording track and press Record. Recording starts after an eight-beat
  count-in. Stop and save the take as a WAV file.

## Album Review

Album Review is a library of deep-listening analyses of albums: for each
album the metadata, an overview, and for each track its energy, tags, part in
the album, and a timeline of timestamped events. Reviews of your own albums
and songs use the same form, so you can compare your work with the reference
albums. Select the record icon in the tool rail to open it.

- The grid shows one card for each album. Search finds titles, artists, and
  genre tags and ignores accents: "oxygene" finds "Oxygène". Artist filters
  show the albums of any selected artist; genre filters show the albums that
  have every selected tag, and a tag that would leave no album is disabled.
  The genre row shows the six most-used tags; "+N more" shows the others.
  Active filters show as chips; Clear filters removes them and the search.
  Sort by year, artist, title, or track count. The count reads "visible /
  total albums".
- Select a card for its page: the metadata, the overview, the listening
  links, a timeline chart with one row for each track, and each track's
  events. Select a timeline row to go to its track.
  Opening an album or returning to the grid starts at the top of that view.
- Review files contain links to cover art. **Display cover art** downloads the
  album's cover from its source and keeps it in the cover cache of the data
  folder, up to 256 MB; the least recently viewed covers go first. A cover
  that cannot be downloaded leaves the placeholder and shows the reason.
  **Clear cover cache** removes the downloaded covers after cover loading
  finishes. **Reload reviews** reads the folder again after its files change
  and finds a missing folder after you restore it. **Credits and rights** names the
  sources.
- The reviews are JSON files in `assets/album-review/` beside the application.
  Without that folder the window shows **No reviews folder**. Agents add and
  change reviews through [agent control](../AGENT-CONTROL.md); the
  [writing guide](../ALBUM-REVIEW-WRITING.md) gives the content rules, and the
  [research shortlist](../ALBUM-REVIEW-RESEARCH.md) lists albums to review next.

## Save and export

Save songs as `.mixjam` files. Export writes stereo PCM16 WAV at 44.1 kHz.
If the used native samples of one product are on different volumes, saving
keeps copies of those WAV files together in the application's data folder.
The song retains its native sample identities. These copies are source audio
for the saved song; repeated saves reuse unchanged copies.

A `.mixjam` song is JSON, format version 24. [SONG-FORMAT.md](../SONG-FORMAT.md)
describes it and how earlier versions open. A song links to WAV files. It is
not a portable audio bundle. A song finds a WAV that moved within a library
folder, or to another library folder, by the end of its path: the file name and
the folders just above it. After moving a library folder, add its new location
before you open the song.

To open a song without **Open**, drag its file from Explorer onto the window.
A dropped file opens like a song chosen with **Open**: unsaved work asks to be
saved first. If you drop several files, the first song among them opens.
