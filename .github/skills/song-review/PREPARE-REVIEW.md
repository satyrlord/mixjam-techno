# Prepare the review

This file holds the body of step 2 of the [song-review skill](SKILL.md).

1. Read `AGENTS.md`, `.env.example`, and `tools/song/README.md`.
   Completion criterion: You know the tools and the `.env.local` keys that they
   need.
2. Read the artist profile at the path that `ARTIST_PROFILE` sets.
   Completion criterion: You know the style, the rules, the targets, the
   reference sheet, and the maximum number of rounds.
3. If there is no artist profile, use the
   [generic targets](REFERENCE-ROUNDS.md#generic-targets). Use a
   reference-profile target where a configured reference profile gives one.
   Completion criterion: Your report and each brief name the source of each
   target. They say when there is no artist profile.
4. Select a private review folder for the song under `tmp/`.
   Completion criterion: The folder holds the round log, or you made the folder.
5. Copy the song and its `.meta.json`, if present, into the review folder.
   Do all work on this working copy, because `render.py --fit-gain` changes
   the song that it gets.
   Completion criterion: The original song and its metadata are unchanged.
6. If the song's `formatVersion` is below 18, convert the working copy to the
   [current song format](../../../docs/SONG-FORMAT.md). Do the same if its
   linked WAVs need library folders. Use the steps in
   [Earlier songs](../../../tools/song/WRITING-METHODS.md#earlier-songs).
   If a source is missing, or you cannot confirm a sample conflict, report a
   blocker. Do not use a different sound in its place.
   Completion criterion: The converted copy has the expected clips and
   sources, or your report names each blocker.
7. Find the song's
   [standing notes](REFERENCE-NOTES.md#standing-notes). If the owner gave
   judgements on the song and there is no file, make one. Use the round log or
   the handoff.
   Completion criterion: Each brief names the standing notes file, or the song
   has no owner judgements yet.
8. Find the song's [level history](REFERENCE-NOTES.md#level-history). If a
   control moved in two directions over the passes and there is no file, make
   one. Use the round log or the handoff.
   Completion criterion: Each brief names the level history file, or no
   control moved back.
9. Find the last version that the owner approved by ear, and its master
   render.
   Completion criterion: Each critic brief names that render, or the song has
   no approved version yet.
10. Find the reference sheet that the artist profile names.
    Completion criterion: Each brief names the reference sheet, or the profile
    names none.
