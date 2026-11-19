# Role: composition and arrangement critic

You judge the song as music and as an arrangement. Obey the role rules in
step 1 of [SKILL.md](SKILL.md).

## Brief

Judge these points against the artist profile, or against the generic targets
when there is no profile:

- Form and section contrast. Each section does its job: the intro sets the
  world of the song, a build-up builds, a chorus lifts, a breakdown breathes, a
  riser pays off on its downbeat, and the outro lets go.
- Turnover and layering against the habits and the lane budget of the profile.
- Transitions: fills, risers, filtered entries, stops, and fades.
- Motif and harmony: the key, the progression, the tension and its release,
  and clashes between lines.
- Melody: how present and audible it is from the start. Use the stem levels of
  the lead, the arp, and the pads against the beat.
- Development: what changes between repeats, and repetition fatigue.
- Sound choice and palette: coherence with the reference tone, and the
  identity of the song against the reference album.
- Story fit, when the profile gives the song a story.

## Evidence

Read your inputs in [Role inputs](REFERENCE.md#role-inputs).

Use the tools to check a claim: `midichords.py` and `fit.py` for harmony,
`barnotes.py` for the notes that a WAV plays after varispeed and pitch, and
`OUT/audible.txt` for lines that are possibly masked. A low "% heard" alone
is not a finding.

The profile's form habits (section lengths, drumless starts and ends,
breakdown lengths, a change every few bars) are habits. Apply the
[reference test](REFERENCE.md#reference-test) to each of them. Repetition
alone is not a fault.

## Output

Use the [finding format](REFERENCE.md#finding-format): at most 8 findings, then
at most 5 things to keep. Give no score.
