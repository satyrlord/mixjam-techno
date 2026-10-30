# Album Review writing guide

This guide gives the rules for the album analysis content in
`assets/album-review/<id>.json`.
It covers the overviews, the track roles, the tags, the energy levels, and the timeline events.

This guide is about the prose only. The full procedure to add an album is in the
[album-analysis skill](../.github/skills/album-analysis/SKILL.md).
That procedure includes the metadata sources, the listening links, and the validation.

The source of truth is the set of high-quality entries in the `assets/album-review/` directory.
If this guide and the data do not agree, change the guide to agree with the data.

## Related Files

- [Review format and rules](../.github/skills/album-analysis/REFERENCE.md)
- [Example: Oxygène](../assets/album-review/jean-michel-jarre-oxygene.json): the benchmark for detail at the instrument level
- [Example: Voices](../assets/album-review/vangelis-voices.json): the benchmark for vocal and choral analysis
- [Example: The Fat of the Land](../assets/album-review/the-prodigy-fat-of-the-land.json): the benchmark for electronic and rave albums
- [Example: MCMXC a.D.](../assets/album-review/enigma-mcmxc-a-d.json): the benchmark for sample-based albums and suites in more than one part

---

## Global Style Rules

These rules apply to every prose field: `overview`, `role`, the event `description`, and `detail`.

- **Third person or noun phrase.** Do not use the first person.
- **No promotional words.** You can use "iconic" and "legendary" when the history supports them.
  Do not use "amazing", "incredible", or "stunning".
  Use precise nouns and verbs, not adjectives.
- **Direct statements.** Do not write "it seems like", "possibly", or "might be". State the analysis directly.
- **Technical words.** Use technical terms, for example `arpeggiated`, `polyrhythmic`, `resonant low-pass filter`,
  `four-on-the-floor`, and `tritone`. Do not explain basic terms.
- **No loose ends.** Each claim must be complete in itself.
  - If you name a series, a trilogy, or a group, list all of its members.
  - If you refer to an earlier or a later album, give its name.
  - The reader must not need to ask "which one?" or "what was the third?" at the end of a sentence.
- **Punctuation.** Use an em dash (`—`) to add a parenthetical expansion.
  Use a comma before a subordinate clause.
  Use a colon before a list or an example.
- **Numbers.** Write timestamps in the `M:SS` or `MM:SS` format.
  Write chart positions as `UK No. 1` and `France No. 4`.
  Write years as four-digit numbers in the text.

---

## Overview Field

The overview is an analytical statement of 3–8 sentences. It is not a promotional text.
About two thirds of the entries use one paragraph.
The other entries use two paragraphs, with a literal `\n\n` in the JSON string between them.
Two paragraphs usually separate the recording context from the commercial reception.

**Required content:**

1. Recording context: where, when, and with which limits (studio, home, budget, schedule).
2. Important equipment or method: give the names of the specific instruments, samplers, or software.
3. Commercial and critical context: chart positions, certifications, awards, and cultural importance.
4. Structural design: how the arc of the album works at the large scale (suites, sides, interludes, continuous flow).
   Most entries include this item. Omit it only if the album has no important structure.

**Timestamp caveat:** If you estimate the event timestamps proportionally, add a short paragraph at the end of the overview.
This paragraph states the margin, for example `Timestamps are approximate to ±5 seconds.`
If the timestamps are exact, do not add this paragraph.

**Good example (Oxygène):**

> Recorded in Jarre's Paris apartment kitchen on Rue de la Trémoille — no
> studio time, no recording budget, no label commission. Six continuous
> parts that function as a single atmospheric statement about air, breath,
> and the texture of space. Key equipment: ARP 2600, EMS Synthi AKS, EMS
> VCS 3, Farfisa Professional Organ, Eminent 310 Unique (the atmospheric
> string sound), Korg Mini-Pops 7 drum machine…

**Avoid:**

> This is a landmark album that changed the face of electronic music forever.

---

## Track `role` Field

The role is one to three sentences. It identifies the function of the track in the album arc.
Start with the structural function.
Add historical or musical context only if the track is important enough to need it.

- Start the role with the prefix `Album role: …`. Saving a review enforces this prefix.
- State *what the track does in the structure*, not how it sounds.
- Prefer noun phrases or subordinate clauses to full sentences with a predicate.
- Mention a chart position, a single release, or a guest vocalist only if it is directly related to the structural role of the track.

**Good examples:**

> Album role: The album's defining statement — the most recognised synthesizer melody of the 1970s.
> Album role: The dramatic gear shift. After 10 tracks of escalating intensity, the album drops into a hazy, downtempo world.
> Album role: Side Two's climax and the album's second single — released as "Part 4 Remix" in November 1981.

---

## Track `tags` Field

Give 2–5 short descriptors in Title Case.
Do not add an energy tag. The album page shows `Energy: <level>` from the `energy` field as the first tag, for example `Energy: Peak`.
An energy tag in `tags` shows the same data two times.

- Write tags as noun phrases, not as sentences.
- Use these types of tags:
  - Instrument names: `ARP 2600`, `Mellotron`.
  - Structural descriptions: `Cumulative Build`, `Side One Centrepiece`, `Three-Part Suite`.
  - Genre labels: `Jungle`, `Big Beat`.
  - Contextual facts: `Lead Single`, `Arthur C. Clarke Ref.`.
  - Techniques: `Sample Collage`, `Vocoder`, `Gregorian Chant Sample`.
- Do not use a general adjective, for example "energetic" or "beautiful", as a tag by itself.
- Saving stops if a track has fewer than 2 tags or more than 5 tags.
- An unfinished entry can have the placeholder tags `Scaffold` and `Tags Pending`. Replace both. Saving stops if a placeholder tag stays.

---

## Energy Field

Give exactly one of these values: `low`, `mid`, `high`, or `peak`.

| Value | Use when the track is |
| --- | --- |
| `low` | Ambient, interlude, drone, spoken word, slow build, downtempo, field recording |
| `mid` | Mid-tempo groove, funk, trip-hop, measured progression, moderate breakbeat |
| `high` | Rave, punk, metal, breakbeat, big beat, assertive electronic |
| `peak` | Major climax, maximum intensity, summit track, final assault |

A track has one energy value. Use the highest level in the track, not the average level.
Use `peak` rarely. Give it only to a true summit. In the current data, about 1 track in 17 is `peak`.

---

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
