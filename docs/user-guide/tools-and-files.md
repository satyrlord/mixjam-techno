# Sound tools, Album Review, and saving

## Sound tools and recording

- To add a WAV file to the Wave category, select **Import** and enter its
  source tempo. If the file belongs to another category, its entry there stays
  available.
- **Edit** opens Sample editor. Use it to trim a sound and to change its pitch.
  The range includes the start frame and excludes the end frame. The buttons
  beside a value change it by one frame or 0.1 semitone. Hold Shift for 100
  frames or one semitone. To type an exact value, click the value. **Listen
  edit** plays the draft. **Save edited sound** writes a new WAV file in the
  Wave category.
- **Stretch** opens Time Stretcher, which fits a sound to a tempo. **Effects**
  opens Effect Studio, which applies effects to a sound. Both save a new WAV
  sample.

### Step synth

Step synth is a board of the lower panel. It plays a source sound as a
sequence of notes on sixteenth steps. A pattern has from 1 to 64 steps: a
maximum of four bars. A wide panel shows all four bars at the same time.

To make a sound:

1. Choose the source: select **Source file…**, or select a sound in the
   Sounds panel and then **Palette sound**. A new source sets **Start** to
   its first hit and turns **Loop source** off.
2. Set **Length**, under Pattern, or drag the end marker on the step ruler.
   **×2** doubles the length and **÷2** halves it. If the new steps are
   empty, **×2** repeats the loop into them. Steps after the length keep
   their notes.
3. To add or remove a note, click a step on the note grid.
4. To hold a note over more steps, drag its last step.
5. To accent a note, click its dot in the **Accent** lane under the grid. To
   slide a note into the next note, click its dot in the **Slide** lane.
6. To set the level of each note, drag in the **Velocity** lane.
7. To hear the pattern, select **Listen** at the left of the bar buttons, or
   press L. Select **Stop**, or press L again, to stop it.
8. Select **Save sound**, then choose a WAV path.

If the pattern peaks over full scale, **Listen** and **Save sound** turn all
of it down to full scale. Accents and filter drive can cause this. The status
line tells how many dB.

**Save sound** writes the WAV file and adds it to the Step synth category.
Beside the WAV file, it writes the pattern as `Name.pattern.json`. **Load**,
under Pattern, opens this file. The arrangement uses it to show the clips of
the sound as notes. **Save**, under Pattern, saves only the editable pattern
as JSON. If the BPM of a loaded pattern is not a tempo choice, it moves to the
nearest choice. The status line tells you. If the application cannot save the
pattern, it shows the error and does not change the existing files.

Other Step synth controls:

- **Gate** sets how much of its last step a note plays, in percent. With
  swing, the step is the swung one, so a note on the beat plays longer than
  a note off the beat.
- **Swing**, **Accent**, and **Slide time** set the timing and the voice.
- **Scale** and its root, in the top row of the grid, shade the pitch rows.
  All notes stay playable. **Randomise**, beside them, writes a new line in
  the scale for the length of the pattern.
- The grid opens on the octave of the notes. The olive **0** row is the
  source pitch.
- To apply a lane edit to many steps, drag on the step ruler to select them.
- **Copy to 2** (or 3, or 4) copies the bar in view onto the next bar.
  **Clear bar** clears the bar in view.
- The undo and redo keys after **Notes** and **Effects** take back and put
  back the last edit of the board. The status line names the edit. With
  the pointer over the board, or the keyboard focus in it, Ctrl+Z and
  Ctrl+Y do the same. Elsewhere they act on the song.
- **Fill** copies the bar in view onto every bar. **Clear**, under Pattern,
  clears every bar.
- **Effects** shows the effects of the pattern. **Notes** shows the note grid
  again. In the effects view, the strip beside **Listen** shows the step that
  plays. The pattern plays as a loop, so the echo and filter tails at its end
  return at its start. **Delay (beats)** of the echo is from 1/16 to 2
  beats. A shorter delay in an older pattern file opens at 1/16 beat.
  **Cutoff** and **Color** show their values in Hz.

### Recording

1. Select a recording track.
2. Press Record. The recording starts after an eight-beat count-in.
3. Stop the recording.
4. Save the take as a WAV file.

If the recording buffer fills, recording stops. You can save the audio recorded
before it stopped.

## Album Review

Album Review is a library of deep-listening analyses of albums. Each album has
its metadata and an overview. Each track has its energy, its tags, its part in
the album, and a timeline of events with timestamps. Reviews of your own albums
and songs use the same form. Thus you can compare your work with the reference
albums. To open Album Review, select **Review**, the record and tonearm in the
tool rail.

### The grid of albums

The grid shows one card for each album.

- The search finds titles, artists, and genre tags, and it ignores accents:
  "oxygene" finds "Oxygène".
- The artist filters show the albums of each selected artist.
- The genre filters show the albums that have all the selected tags. The
  application disables a tag that leaves no album. The genre row shows the six
  most used tags. "+N more" shows the other tags.
- Active filters show as chips. **Clear filters** removes them and the search.
- You can sort by year, artist, title, or track count. The count shows
  "visible / total albums".

### The page of an album

To open the page of an album, select its card. The page shows the metadata,
the overview, and the listening links. It also shows a timeline chart with one
row for each track, and the events of each track. To go to a track, select its
timeline row. When you open an album or return to the grid, the view starts at
the top.

### Cover art and the reviews folder

- Review files contain links to cover art. **Display cover art** downloads the
  cover of the album from its source. The application keeps the cover in the
  cover cache of the data folder, up to 256 MB. When the cache is full, the
  covers that you viewed least recently go first. If the application
  cannot download a cover, the placeholder stays and shows the reason.
- **Clear cover cache** removes the downloaded covers after all covers
  load.
- **Reload reviews** reads the folder again after its files change. It also
  finds a missing folder after you restore it.
- **Credits and rights** names the sources.

The reviews are JSON files in `assets/album-review/` beside the application.
Without that folder, the window shows **No reviews folder**. Agents add and
change reviews through [agent control](../AGENT-CONTROL.md). The
[writing guide](../ALBUM-REVIEW-WRITING.md) gives the content rules. The
[research shortlist](../ALBUM-REVIEW-RESEARCH.md) lists the albums to review
next.

## Save and export

Save songs as `.mixjam` files. Export writes stereo PCM16 WAV at 44.1 kHz.

A song can use native samples of one product that are on different volumes.
Then the save keeps copies of these WAV files together in the data folder of
the application. The song keeps its native sample identities. These copies
are the source audio of the saved song. Later saves use the copies again if
they did not change.

A `.mixjam` song is a JSON document that links to WAV files.
[SONG-FORMAT.md](../SONG-FORMAT.md) gives the current format version and
describes how earlier versions open.

A song finds a WAV file again by the end of its path: the file name and the
folders just above it. This works when the file moved in a library folder or to
a different library folder. If you move a library folder, add its new location
before you open the song.

To open a song without **Open**, drag its file from Explorer onto the window.
The song opens as if you chose it with **Open**. If there is unsaved work, the
application asks to save it first. If you drop two or more files, the first
song among them opens.
