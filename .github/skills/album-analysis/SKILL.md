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
application checks every rule before it writes the file, so a saved review is
a valid review.

If the request has more than one album, use one sub-agent for each album and
give it this skill and one album. Then compare and correct their results
before you save the reviews.

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
   Completion criterion: You know whether the review exists. If it exists, use
   `review id=ID` and plan an update, not an add.

## Step 2: Gather the metadata

1. Run `python -I tools/album-review/lookup.py release "ARTIST" "TITLE" --year YEAR`.
   Choose the official release that matches the album.
2. Run `python -I tools/album-review/lookup.py tracks MBID` for the track
   list, the durations, and the runtime.
3. Read the album's Wikipedia article for the year, the label, the producers,
   the genre, and the recording and production context.
4. Write the metadata fields in the formats of the "Metadata Fields" section
   of the writing guide.

**Completion criterion:** every metadata field has a value from a source that
you can name. `label` and `producer` are `"Unknown"` only when no source
gives the value.

For own work, the local masters and the song files are the source. Measure
the durations from the masters. Do not invent a release that does not exist.

## Step 3: Find the listening links

Listening links are optional. Add only links to official releases:

- `youtubeUrl`: an https link on `www.youtube.com`, `music.youtube.com`, or
  `youtu.be`.
- `bandcampUrl`: an https link on `bandcamp.com` or an artist subdomain. The
  owner's albums are on `https://eklipse-music.bandcamp.com/`.
- Own work only: `localAudio` (a WAV master) and `songFile` (a `.mixjam` or
  `.mix` song), for the album or song and for each track.

You can read the text of these pages (titles, track lists, durations), not
the audio. For own work, analyse the local masters.

**Completion criterion:** your report names each link field that stays empty.

## Step 4: Find the cover link

1. Run `python -I tools/album-review/lookup.py cover "ARTIST" "TITLE"`.
   If the search picks the wrong article, add `--page "ARTICLE TITLE"`.
2. Open the image address and the article, and make sure that they show this
   album's front cover.
3. Put the image address in `coverUrl` and the article in `coverSource`.

The tool finds the link but does not download the image. The application
downloads a cover only when a person selects **Display cover art**. Do not
copy a cover image into the repository. If no source gives a reliable cover,
leave `coverUrl` out: the window shows its placeholder.

**Completion criterion:** `coverUrl` is the image of this album, or the review
has no `coverUrl`.

## Step 5: Write the analysis

Write the `overview` and, for each track, the `energy`, `tags`, `role`, and
`events`. Obey every rule in `docs/ALBUM-REVIEW-WRITING.md`.

**Completion criterion:** all of these are true:

- No placeholder text stays in the review.
- Every track has a `role` that starts with `Album role: `.
- Every track has 2–5 `tags`, and no tag shows the energy level.
- Every track has a number of `events` that agrees with the event table of
  the writing guide, in time order and before the end of the track.
- Every `timestamp` uses `M:SS` or `MM:SS`.

## Step 6: Save

1. Write the review to a JSON file in `tmp/`.
2. Send `add_review file=PATH` for a new review, or `update_review file=PATH`
   for an existing one.
3. If the reply is an error, it names each broken rule. Correct the review and
   send it again.

**Completion criterion:** the reply has `"ok":true` and the `path` of the
saved file.

## Step 7: Check the window

1. Click **Album Review** in the tool rail, or send
   `click target="Album Review"`.
2. Open the review's card and read the page.

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
