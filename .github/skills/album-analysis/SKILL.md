---
name: album-analysis
description: >
  Write or finish an Album Review: a deep-listening analysis of an album, or
  of the owner's own album or song, saved through the agent channel. Use when
  the user asks to add or correct a review, or to review their own work.
---

# Album analysis

Each run makes one **review**: one `assets/album-review/<id>.json` file that
the application accepted through `add_review` or `update_review`. The
application checks the format and its validation rules before it writes the
file. Check the factual claims and the writing guide yourself.

If the request has more than one album, prepare one draft per album. When
sub-agents are available, give each this skill and one album. Ask for a draft
and its evidence only. A sub-agent must not send `add_review` or
`update_review`. Compare and correct the drafts, then save and check each
review yourself. If sub-agents are unavailable, prepare the drafts in sequence.

Read [REFERENCE.md](REFERENCE.md) for the fields, the id format, the rules,
and the commands. Read `docs/ALBUM-REVIEW-WRITING.md` for the prose, the
tags, the energy levels, and the timeline events.

## Step 1: Start the session

Use the `operate-app` skill to start an isolated session with
`MIXJAM_TECHNO_DATA_DIR` in `tmp/`. The reviews folder is
`assets/album-review/` of the checkout that the executable runs from, not the
data folder. A saved review is a repository file, so commit it only when the
user asks.

1. Send `reviews search="TITLE"` and `reviews artist="ARTIST"`.
   Check `total` and read all result pages. Check `problems` for unreadable files.
   Completion criterion: You know whether the review exists, or report an
   unreadable folder or file as a blocker. If the review exists, use
   `review id=ID` and plan an update, not an add.

## Step 2: Gather the metadata

1. For a released album, run
   `python -I tools/album-review/lookup.py release "ARTIST" "TITLE" --year YEAR`.
   Omit `--year YEAR` when the year is unknown.
   Completion criterion: You select the official release that matches the album.
2. For that release, run `python -I tools/album-review/lookup.py tracks MBID`.
   Completion criterion: You have the track list, durations, and runtime, or
   have identified the values that need another source.
3. Read sources for the year, label, producers, genre, and production context.
   Use the album's Wikipedia article when it exists, and other reliable sources
   when it does not supply the facts.
   Completion criterion: Each factual value has a named source.
4. Write the metadata fields in the formats of the "Metadata Fields" section
   of the writing guide.
   Completion criterion: Each required field follows the format and has a
   supported value, or uses `"Unknown"` where the format permits it.

**Completion criterion:** every metadata field has a value from a source that
you can name. `label` and `producer` are `"Unknown"` only when no source
gives the value.

For unpublished own work, use the local masters, song files, and owner-provided
facts. Measure the durations from the masters. Skip the release lookup when no
release exists. Do not invent missing facts.

## Step 3: Find the listening links

Listening links are optional. Add only links to official releases:

- `youtubeUrl`: an https link on `www.youtube.com`, `music.youtube.com`, or
  `youtu.be`.
- `bandcampUrl`: an https link on `bandcamp.com` or an artist subdomain. The
  owner's albums are on `https://eklipse-music.bandcamp.com/`.
- Own work only: `localAudio` (a WAV master) and `songFile` (a `.mixjam` or
  `.mix` song), for the album or song and for each track.

Read the text of these pages for titles, track lists, and durations. If audio
is not available to you, do not claim that you listened to it. For own work,
analyse the local masters when the tools can read them.

**Completion criterion:** your report names each link field that stays empty.

## Step 4: Find the cover link

1. Run `python -I tools/album-review/lookup.py cover "ARTIST" "TITLE"`.
   If the search picks the wrong article, add `--page "ARTICLE TITLE"`.
   Completion criterion: You have a candidate cover and source, or the lookup
   found no candidate.
2. If the lookup found a candidate, open the image address and the article.
   Completion criterion: The source identifies the album and its cover, or
   you reject the candidate.
3. For a verified candidate, put the image address in `coverUrl` and the article
   in `coverSource`.
   Completion criterion: Both fields identify the verified source, or neither
   field is added.

The tool finds the link but does not download the image. The application
downloads a cover only when a person selects **Display cover art**. Do not
copy a cover image into the repository. If no source gives a reliable cover,
leave `coverUrl` out: the window shows its placeholder.

**Completion criterion:** `coverUrl` is the image of this album, or the review
has no `coverUrl`.

## Step 5: Write the analysis

Write the `overview` and, for each track, the `energy`, `tags`, `role`, and
`events`. Obey every rule in `docs/ALBUM-REVIEW-WRITING.md`.
Use source or measurement evidence for each musical claim. If the available
evidence cannot support the requested detail, stop and report the missing evidence.
Do not invent instruments, events, or timestamps.

**Completion criterion:** all of these are true:

- No placeholder text stays in the review.
- Every track has a `role` that starts with `Album role: `.
- Every track has 2–5 `tags`, and no tag shows the energy level.
- Every track has a number of `events` that agrees with the event table of
  the writing guide, in time order and before the end of the track.
- Every `timestamp` uses `M:SS` or `MM:SS`.

## Step 6: Save

1. Write the review to a JSON file in `tmp/`.
   Completion criterion: The file contains the full review.
2. Send `add_review file=PATH` for a new review, or `update_review file=PATH`
   for an existing one.
   Completion criterion: You have a reply for the intended review.
3. If the reply is an error, correct the reported problem when it is in scope.
   Completion criterion: The problem is corrected, or you report the blocker.
4. After a correction, send the review again.
   Completion criterion: The application accepts the review, or you report the
   error that blocks it.

**Completion criterion:** the reply has `"ok":true` and the `path` of the
saved file.

## Step 7: Check the window

1. Click **Album Review** in the tool rail, or send
   `click target="Album Review"`.
   Completion criterion: The review window opens.
2. Open the review's card and read the page.
   Completion criterion: The card opens the intended review.

**Completion criterion:** the grid shows the album's card, and its page shows
the metadata, the overview, the listening links, and every track with its
timeline.

## Step 8: Update the research shortlist

Remove the album from `docs/ALBUM-REVIEW-RESEARCH.md`. Find the list item with
the same artist and album title.

**Completion criterion:** the shortlist has no item for the reviewed album.

## Read reviews for song building

A song-building agent can read the reviews without the window:

- `reviews tag=Acid energy=peak` lists the albums with such a track.
- `review_tracks tag=Breakbeat` gives each matching track with its role and
  its timestamped events: a section plan that you can measure against.
- `review id=ID` gives one review in full.
- `reviews kind=own` lists the owner's own work, to compare with the
  reference albums.
