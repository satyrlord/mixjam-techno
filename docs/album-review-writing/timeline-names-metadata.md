# Album Review writing guide: timeline events, names, and metadata

## Timeline Event Fields

Use a number of events that agrees with the length of the track:

| Track length | Events |
| --- | --- |
| Interlude or short track (less than approximately 3 min) | 2–4 |
| Standard track (3–10 min) | 4–8 |
| Extended track (more than 10 min) | 8–12 |

Saving requires 2–14 events in each track.
The maximum in the current data is 14 events, in a standard track (`Break & Enter`, 8:24).
The maximum for an extended track is 13 events (`Les Chants Magnétiques Part 1`, 17:50).

### `timestamp`

Use the `M:SS` or `MM:SS` format. Do not use only seconds. Do not use hours.
You can estimate the values proportionally. If you do, state the margin in the caveat paragraph of the overview.

Put the events in time order. Each timestamp must be after the timestamp before it and before the end of the track.
The timeline chart draws a segment from each timestamp to the next one. Saving a review enforces this rule.

### `section`

Write a short label for the structural moment. Use a few words, not a sentence, and no full stop at the end.
The data uses two styles. Both styles are correct:

- **Structural label.** This is the most frequent style: `Intro`, `Build`, `Drop`, `Breakdown`, `Peak`, `Outro`,
  `Bridge`, `Chorus`, `Verse`, `Crossfade`, and compounds such as `Final Drop`, `Extended Peak`, `Vocal Hook`,
  `Beat entry`, and `Choir Entry`.
- **Named-event label.** Use this style for analysis at the instrument level, as in Oxygène:
  `Eminent 310 string pad`, `Mellotron enters`, `Gregorian chant`.

The data does not use one capitalization rule (`Beat entry` and `Choir Entry`).
Choose one rule for each album and use it for all of the album.

### `description`

Write one sentence or clause. It tells *what occurs in the structure or the music* at this timestamp.
You can add a second short fragment for emphasis.

- Use the present tense. Use the active voice or a noun phrase.
- Be technical and specific. Name the instrument. Describe the change in rhythm or harmony.
- You can start with a lowercase letter if the description continues the section label.
  For example, `section: "Intro"` → `description: "sub-tonal drone and distant chord cluster…"`.
  More than a quarter of the events in the data use this continuation style.
  Both capitalization styles are correct. Do not mix them without a reason in one track.

**Good:** `Eminent 310 string pad enters — the warm, sustained chord establishing the album's harmonic language.`

**Avoid:** `A really nice synth sound comes in here and it creates a great atmosphere.`

### `detail` (optional)

Write 1–3 sentences that give more context than the description. Use this field for:

- Equipment notes: the model name, its origin, and the reason for the choice of that instrument.
- Production technique: how the effect was made.
- Historical or cultural context: the first use of a technology, the cultural reception, or a note about a live performance.
- Structural function: why this moment is important in the larger arc.

If you have nothing to add to the description, omit the field.

---

## Instrument and Sample Names

Give the names of specific instruments and models when you know them. This is a main feature of the analysis style.

- Use exact commercial names: `ARP 2600`, `EMS Synthi AKS`, `Yamaha CS-80`, `Roland TR-808`, `Akai S1000`, `Fairlight CMI`.
- For samplers or DAWs, give the name of the platform. Give the version if it is relevant: `Akai MPC 3000`, `Ableton Live`.
- For sampled material, give the source. Include the performer, the work, and the year when you know them:
  `groove closely modelled on Soul II Soul's 'Keep On Movin'' (1989)`,
  `sampled Capella Antiqua München vocal — taken from 'Procedamus in Pace'`.
- If the instrument is the main part of the character of the track, explain *why* it sounds as it does.
  For example, explain the filter character, the oscillator type, or the historical context.
- If you know the specific instrument, do not use a general term. Write `Eminent 310 Unique`, not "string synthesizer".

---

## Metadata Fields

- `label`: the primary label of the original release.
  For a joint release, put ` · ` (space, middle dot, space) or ` / ` (space, slash, space) between the labels:
  `"Disques Motors · Polydor"` or `"Blizzard Entertainment / Sierra On-Line"`.
- `producer`: use the same format for co-producers: `"Trevor Horn · Tom Newman · Mike Oldfield"`.
- Do not write an empty string `""` in `label` or `producer`. If the value is not known, write `"Unknown"`.
- `genre`: the short display string. Put ` / ` between the primary subgenres: `"Electronic / Ambient"`.
- `genreTags`: 1–9 selected filter tags.
  - Include the primary values from `genre` if they are useful filters.
  - Add wider or more specific categories if they help users find albums.
  - Start this array from the parts of `genre`. You can then change it.
  - The Album Review window counts spelling variants of one tag (for example `New Age` and `New-Age`) as one tag.
    It shows the form that most albums use.
- `runtime`: the total length of the album.
  Use `M:SS` or `MM:SS` below one hour, for example `42:17`. Use `H:MM:SS` from one hour, for example `1:05:33`.
  Do not write a runtime of one hour or more in minutes only, for example `65:33`.
  The runtime must be within 30 seconds of the sum of the track durations. Saving a review enforces this rule.
  If the runtime and the track sum are more different, a track duration or the runtime is wrong. Check both against MusicBrainz.
