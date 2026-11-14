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
Use one paragraph, or two separated by a literal `\n\n` in the JSON string.
Two paragraphs can separate the recording context from the commercial reception.

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

## Timeline events, names, and metadata

The rules for the timeline event fields, instrument and sample names, and the metadata fields are in
[the second part of this guide](album-review-writing/timeline-names-metadata.md). They are part of this guide: obey both files.
