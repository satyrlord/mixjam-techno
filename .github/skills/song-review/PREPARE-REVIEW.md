# Prepare the review

This file holds the body of step 2 of the [song-review skill](SKILL.md).

1. Read `AGENTS.md`, `.env.example`, and `tools/song/README.md`.
   Completion criterion: You know the tools and the `.env.local` keys that they
   need.
2. Read the artist profile at the path that `ARTIST_PROFILE` sets. An
   environment variable overrides `.env.local`.
   Completion criterion: You know the style, the rules, the targets, the
   reference sheet, and the maximum number of rounds.
3. If there is no artist profile, use the
   [generic targets](REFERENCE-ROUNDS.md#generic-targets). Use a reference-profile
   target where a configured reference profile gives one.
   Completion criterion: Your report and each brief name the source of each
   target, and say when there is no artist profile.
4. Select a private review folder for the song under `tmp/`.
   Completion criterion: The folder holds the round log, or you made the folder.
5. Copy the song and its `.meta.json`, if present, into the review folder.
   Do all work on this working copy. `render.py --fit-gain` changes the song
   that it gets.
   Completion criterion: The original song and its metadata are unchanged.
6. If the song's `formatVersion` is below 18, or its linked WAVs need library
   folders, convert the working copy to the
   [current song format](../../../docs/SONG-FORMAT.md). Set
   `MIXJAM_TECHNO_DATA_DIR` to a private folder in the review folder, then run
   `.\target\release\mixjam-techno.exe convert --mix INPUT --output OUTPUT.mixjam`
   with one `--samples FOLDER` for each necessary WAV folder. Copy the
   metadata beside the converted song with the matching `.meta.json` name.
   Completion criterion: The converted copy has the expected clips and
   sources. Each missing source or unconfirmed sample conflict is in your
   report as a blocker. Do not use a different sound in its place. Keep the
   data folder while the converted song needs its files.
7. Find the song's [standing notes](REFERENCE-NOTES.md#standing-notes). If the owner
   has given judgements on the song and there is no file, make it from the
   round log or the handoff.
   Completion criterion: Each brief names the standing notes file, or the song
   has no owner judgements yet.
8. Find the song's [level history](REFERENCE-NOTES.md#level-history). If a control
   moved in two directions over the passes and there is no file, make it from
   the round log or the handoff.
   Completion criterion: Each brief names the level history file, or no
   control moved back.
9. Find the last version that the owner approved by ear, and its master
   render.
   Completion criterion: Each critic brief names that render, or the song has
   no approved version yet.
10. Find the reference sheet that the artist profile names.
    Completion criterion: Each brief names the reference sheet, or the profile
    names none.

Obey the applicable creative, review, and delivery rules of the artist
profile. They outrank a critic finding and the generic targets. The current
instructions of the user outrank the profile. Do not copy private profile
details into a public file.

Before you open a visible application window or play sound on the owner's
computer, tell the owner what opens, when, if it plays sound, and for how long.
Use headless renders for all measurement.
