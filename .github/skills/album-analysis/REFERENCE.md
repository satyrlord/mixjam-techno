# Album analysis: reference

This file is the reference for the `album-analysis` skill.
`docs/ALBUM-REVIEW-WRITING.md` has the rules for the prose, the tags, the
energy levels, and the timeline events.

## Contents

- [Storage](#storage)
- [Id format](#id-format)
- [Fields](#fields)
- [Requests](#requests)
- [Cover art](#cover-art)
- [Lookup tool](#lookup-tool)

## Storage

- Each review is one file, `assets/album-review/<id>.json`, in the
  repository. The application reads the folder at run time. The executable
  does not contain the reviews.
- The application searches for `assets/album-review` beside its own folder.
  That is the repository root for a build in `target/`, or the installation
  folder. It does not use the data folder. When the folder is missing, the
  window shows **No reviews folder**.
- The files are pretty JSON with two-space indents and a final newline. The
  keys are in the order of the tables below. The application writes this form.
- A file that breaks a rule does not load. The window and the `reviews` reply
  list it with the reason.

## Id format

- `<artist-slug>-<title-slug>`, for example `vangelis-voices` and
  `harold-budd-brian-eno-the-pearl`.
- Only lowercase `a-z`, `0-9`, and single hyphens. Remove accents and
  apostrophes.
- The `id` is the file name without `.json`.

## Fields

Review fields, in file order:

| Field | Status | Rule |
| --- | --- | --- |
| `id` | Required | See "Id format". |
| `kind` | Required | `reference` for another artist's album, `own` for the owner's album or song. |
| `artist` | Required | Non-empty. |
| `title` | Required | Non-empty. |
| `year` | Required | A four-digit number. |
| `label` | Required | Non-empty. `"Unknown"` when no source gives it. |
| `producer` | Required | Non-empty. `"Unknown"` when no source gives it. |
| `genre` | Required | A string, for example `"Electronic / Ambient"`. |
| `genreTags` | Required | 1–9 non-empty strings. |
| `runtime` | Required | `M:SS` or `MM:SS` under one hour, `H:MM:SS` from one hour. Within 30 seconds of the track sum. |
| `isSoundtrack` | Optional | `true` when present. Leave it out for an album that is not a soundtrack. |
| `coverUrl` | Optional | The http or https address of the cover image. |
| `coverSource` | Optional | The http or https address of the page of the image. |
| `youtubeUrl` | Optional | https on `www.youtube.com`, `music.youtube.com`, or `youtu.be`. |
| `bandcampUrl` | Optional | https on `bandcamp.com` or a subdomain of it. |
| `localAudio` | Own work only | The path of a `.wav` master on this computer. |
| `songFile` | Own work only | The path of a `.mixjam` or `.mix` song. |
| `overview` | Required | Non-empty. A blank line (`\n\n`) separates paragraphs. |
| `tracks` | Required | One or more tracks. |

Track fields, in file order:

| Field | Status | Rule |
| --- | --- | --- |
| `num` | Required | The track number, from 1. It is unique within the review. |
| `title` | Required | Non-empty. |
| `duration` | Required | `M:SS` or `MM:SS`. |
| `energy` | Required | `low`, `mid`, `high`, or `peak`. |
| `tags` | Required | 2–5 non-empty strings. |
| `role` | Required | Starts with `Album role:` and a space, and says more. |
| `events` | Required | 2–14 events, in time order, each before the end of the track. |
| `localAudio` | Own work only | The track's `.wav` master. |
| `songFile` | Own work only | The track's song file. |

Event fields: `timestamp` (`M:SS` or `MM:SS`), `section`, and `description`
must be present and non-empty. `detail` is optional and non-empty when present.

A file with an unknown field does not load.

The application also rejects this placeholder text:

- the overview lead `Initial scaffold generated from MusicBrainz metadata.`
- the role `Initial scaffold only. Narrative role pending detailed listen.`
- the tags `Scaffold` and `Tags Pending`
- the event description `Detailed timestamp notes pending.`

The rules do not check the content of the tags or the prose.

## Requests

Send each request with `mixjam-techno agent COMMAND ...` to a session that
started with `--agent` or `--agent-port PORT`. `agent help` gives the full list.

| Request | Reply |
| --- | --- |
| `reviews [search= artist= tag= energy= kind= sort= offset= limit=]` | The folder, the total, and review summaries: id, kind, artist, title, year, genre tags, runtime, track count. |
| `review id=ID` | One review in full. |
| `review_tracks [id= artist= tag= energy= kind= offset= limit=]` | The total and tracks, each with its review id, artist, album, year, kind, and the whole track with its events. |
| `add_review review={...}` or `add_review file=PATH` | Saves a new review. Fails when the id exists. |
| `update_review review={...}` or `update_review file=PATH` | Replaces the review with that id. Fails when the id does not exist. |

- `reviews` and `review_tracks` return up to 50 results by default. Set
  `limit=` to change the page size, and `offset=` to read a different page.
- `search=` looks in the title, the artist, and the genre tags. Case and
  accents do not count.
- `artist=` matches part of the artist's name.
- `tag=` matches a track tag or a genre tag of the album. Case, accents,
  spaces, and hyphens do not count: `four on the floor` matches
  `Four-on-the-floor`.
- `energy=` and `tag=` keep a review that has at least one such track.
- `sort=` is `year` (the default), `year_desc`, `artist`, `title`, or
  `tracks`. Each order breaks ties by year, then artist, then title.
- A long review does not fit on a command line. Write it to a file and use
  `file=`, or send a JSON line on standard input with `agent -`.

## Cover art

- `coverUrl` is a remote address. The repository holds no cover image.
- The window downloads a cover only when a person selects **Display cover
  art**. The download runs on a background thread, with the user agent
  `MixJamTechno/<version>`. It accepts only http and https addresses. It
  accepts only a JPEG, PNG, or WebP image of at most 20 MB. The download ends
  after 20 seconds.
- Downloaded covers go to `album-review/covers/<id>.<ext>` in the data folder
  (`%LOCALAPPDATA%\MixJam Techno`, or `MIXJAM_TECHNO_DATA_DIR`). The cache keeps
  at most 256 MB and removes the least recently viewed covers first. **Clear
  cover cache** removes all of them.
- A failed download keeps the placeholder and shows the reason.

## Lookup tool

`tools/album-review/lookup.py` reads MusicBrainz and Wikipedia with a
descriptive user agent. It needs only the Python standard library. Run it
with `python -I`.

| Command | Output |
| --- | --- |
| `release "ARTIST" "TITLE" [--year YEAR]` | MusicBrainz release candidates, official releases first. |
| `tracks MBID` | The track list with `M:SS` durations, the runtime, and the release date. |
| `cover "ARTIST" "TITLE" [--page "ARTICLE"]` | `coverUrl` and `coverSource` from the album's Wikipedia article. |

MusicBrainz permits one request each second. The tool waits between requests.
If a track duration is not available, that duration and the album runtime show
`?:??`. Find the missing duration in a different source before you use the
metadata in a review.
