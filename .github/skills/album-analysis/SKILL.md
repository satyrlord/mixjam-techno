---
name: album-analysis
description: >
  Add or correct an Album Review: a deep-listening analysis of a released
  album, or of the owner's own album or song, saved through the agent channel.
  Use when the user asks to add a review, to correct a review, or to review
  their own work.
---

# Album analysis

Each run makes one **review**: one `assets/album-review/<id>.json` file that
the application accepted through `add_review` or `update_review`. The
application checks the format and the validation rules before it writes the
file. It does not check the facts or the prose. You must check them.

Read [REFERENCE.md](REFERENCE.md) when you write or check a field, an id, a
request, or a cover link. Before you write the metadata or the analysis, read
`docs/ALBUM-REVIEW-WRITING.md` and its second part,
`docs/album-review-writing/timeline-names-metadata.md`.

A saved review is a repository file. Commit it only when the user asks.

## Branches

| Request | Branch | Differences |
| --- | --- | --- |
| Review a released album | Reference | All steps |
| Correct an existing review | Correction | Step 1 finds the review; steps 2 to 5 apply only to the fields that change; step 7 uses `update_review` |
| Review the owner's own album or song | Own work | Steps 2 and 3 use local files; `kind` is `own` |
| Review more than one album | Several albums | Step 1 once; steps 2 to 6 make one draft for each album; steps 7 to 9 for each draft |

For several albums, if sub-agents are available, give each sub-agent this
skill and one album. Ask it for a draft and its evidence only. A sub-agent must
not send `add_review` or `update_review`. Compare and correct the drafts, then
save and check each review yourself. If sub-agents are not available, prepare
the drafts one after the other.

## Step 1: Start the session

1. Use the `operate-app` skill to start an isolated session, with
   `MIXJAM_TECHNO_DATA_DIR` in `tmp/`.
   Completion criterion: `wait idle=true` replies with `"ok":true`.
2. Send `reviews search="TITLE"` and `reviews artist="ARTIST"`. Read all result
   pages, and compare the results with `total`.
   Completion criterion: You know if the review exists. If `problems` lists an
   unreadable folder or file, report it as a blocker.
3. If the review exists, send `review id=ID`.
   Completion criterion: You have the current review, and you plan an update,
   not an add.

## Step 2: Gather the metadata

1. For a released album, run
   `python -I tools/album-review/lookup.py release "ARTIST" "TITLE" --year YEAR`.
   If you do not know the year, omit `--year YEAR`.
   Completion criterion: You selected the official release that agrees with the
   album.
2. For that release, run `python -I tools/album-review/lookup.py tracks MBID`.
   Completion criterion: You have the track list, the durations, and the
   runtime, or you know which values need a different source.
3. Read sources for the year, the label, the producers, the genre, and the
   production context. Use the Wikipedia article of the album when it exists.
   Use other reliable sources for the facts that it does not give.
   Completion criterion: Each factual value has a named source.
4. For own work, use the local masters, the song files, and the facts that the
   owner gives. Measure the durations from the masters. If no release exists,
   do not do substeps 1 and 2.
   Completion criterion: Each value comes from a local file or from the owner.
5. Write the metadata fields in the formats of the "Metadata Fields" section
   of the writing guide.
   Completion criterion: Each required field has a supported value in its
   format. `label` and `producer` are `"Unknown"` only when no source gives
   the value. A runtime of one hour or more uses `H:MM:SS`, not minutes only.

Do not invent a missing fact.

## Step 3: Find the listening links

1. Find the official release pages for `youtubeUrl` and `bandcampUrl`. Use
   only the hosts that the fields table of REFERENCE.md permits. For own work,
   the `bandcampUrl` of an earlier own review (`reviews kind=own`) gives the
   owner's Bandcamp page.
   Completion criterion: Each link is an official release page. Your report
   names each link field that stays empty.
2. Read the text of these pages for the titles, the track lists, and the
   durations.
   Completion criterion: The track list agrees with the pages, or you report
   the difference.
3. For own work, add `localAudio` (a WAV master) and `songFile` (a `.mixjam` or
   `.mix` song) for the album or song and for each track. Analyse the local
   masters when the tools can read them.
   Completion criterion: Each local path exists on this computer.

If the audio is not available to you, do not say that you listened to it.
Local paths are private to this computer. Do not commit a review with a local
path unless the owner asks.

## Step 4: Find the cover link

1. Run `python -I tools/album-review/lookup.py cover "ARTIST" "TITLE"`.
   If the search finds the wrong article, add `--page "ARTICLE TITLE"`.
   Completion criterion: You have a candidate cover and its source, or the
   lookup found no candidate.
2. Read the article of the candidate.
   Completion criterion: The article is about this album and shows this
   cover. If not, reject the candidate. The lookup can give the cover of a
   different album.
3. Send an HTTP HEAD request to the image address.
   Completion criterion: The status is 200, and the content type is
   `image/jpeg`, `image/png`, or `image/webp`. If not, reject the candidate.
4. For a verified candidate, put the image address in `coverUrl` and the
   article address in `coverSource`.
   Completion criterion: Both fields identify the verified source, or the
   review has neither field.

Do not copy a cover image into the repository. If no source gives a reliable
cover, leave out `coverUrl`. The window then shows its placeholder.

## Step 5: Write the analysis

1. Write the `overview`, and the `energy`, `tags`, `role`, and `events` of each
   track. Obey each rule in `docs/ALBUM-REVIEW-WRITING.md`.
   Completion criterion: Each musical claim has a source or a measurement.
   Do not invent instruments, events, or timestamps.
2. If the evidence cannot support the requested detail, stop and report the
   missing evidence.
   Completion criterion: The review has only supported claims.
3. Check the analysis against these rules.
   Completion criterion: All of these are true:
   - No placeholder text stays in the review.
   - Each track has a `role` that starts with `Album role:` and a space.
   - Each track has 2–5 `tags`, and no tag names the energy level.
   - Each track has a number of `events` that agrees with the event table of
     the writing guide. The events are in time order and before the end of the
     track.
   - Each `timestamp` uses `M:SS` or `MM:SS`.

## Step 6: Write the file

1. Write the full review to a JSON file in `tmp/`.
   Completion criterion: The file has all fields of the review.

## Step 7: Save

1. Send `add_review file=PATH` for a new review, or `update_review file=PATH`
   for an existing review.
   Completion criterion: You have a reply for the intended review.
2. If the reply is an error and the problem is in scope, correct the problem
   and send the review again.
   Completion criterion: The reply has `"ok":true` and the `path` of the saved
   file, or you report the error that blocks the save.

## Step 8: Check the window

1. Click **Album Review** in the tool rail, or send
   `click target="Album Review"`.
   Completion criterion: The Album Review window opens.
2. Open the card of the review and read the page.
   Completion criterion: The grid shows the card of the album. Its page shows
   the metadata, the overview, the listening links, and each track with its
   timeline.

## Step 9: Update the research shortlist

1. In `docs/ALBUM-REVIEW-RESEARCH.md` and the files in
   `docs/album-review-research/`, find the list item with the same artist
   and album title, and remove it.
   Completion criterion: The shortlist has no item for the reviewed album.
