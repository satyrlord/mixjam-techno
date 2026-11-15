# Role: mastering engineer

You explore the master pedalboard on the premaster of the song and recommend
a board. Decide by measurement against the reference profile. Say which
choices need ears. Obey the role rules in step 1 of [SKILL.md](SKILL.md).

## Evidence

Read your inputs in [Role inputs](REFERENCE.md#role-inputs). Also read:

- the board rules of the artist profile;
- the factory presets of the application, in `src/master/presets.rs`. A preset
  sets the settings and the order of the pedals, and switches the booster
  pedals off. It keeps the trim, the bypass, and the keys of the dynamics
  pedals.

The premaster is `OUT/premaster.wav`, with the board and the trim off. Select
the target for each measure in this order:

1. the target of the artist profile;
2. the target of the reference profile that `master.py` reads;
3. if there is no artist profile, the
   [generic target](REFERENCE.md#generic-targets).

If no source gives a target, report the missing target. Name the source of
each target.

## Method

Run `python -I tools/song/master.py PREMASTER BOARD.json OUT_DIR --song SONG`
for each board. The tool renders the board over the premaster. It measures the
loud blocks, the sections, the 1/3-octave balance against the references, and
the drums: how far the board pushes the hits down, and the kick and hat attack
against the premaster. Report the limits of `master.py` in
[Measurement limits](REFERENCE.md#measurement-limits).

1. Measure the premaster and the current board.
   Completion criterion: You know where the mix stands against each target.
2. Try each pedal alone at three or more settings, at the same loudness.
   Completion criterion: A table says what each pedal does to this mix.
3. Measure the factory presets with the trim of the song.
   Completion criterion: You know which presets come close, and why the rest do
   not.
4. Combine the pedals that helped. Fit each candidate to the loudness target
   with `--fit DB`, to compare the boards at equal loudness.
   Completion criterion: Each candidate has a fitted limiter gain and a full measurement.
5. Select the board that meets the most targets without harm to the drums or
   to the section contrast.
   Completion criterion: Each choice has a measured reason.
6. Make a more restrained alternative and a bolder alternative from the same family.
   Completion criterion: Both are fitted and measured.

The limiter is always the last pedal. Keep the order of the board unless a measured
reason needs a change. Do not change the mix.

## Pedal notes

These notes come from measurements of the pedals on one mix. They are a start
for step 2, not a result. Measure the pedals again on this mix.

| Pedal | Notes |
| --- | --- |
| trim | Gain before every pedal. It also sets how hard `boosterCompressor` works. |
| boosterEq | 10 bands, 50 is flat: 60 Hz and 12 kHz shelves (about 0.24 dB per step) and Q 4 peaks from 80 Hz to 9 kHz (about 0.36 dB per step). The only narrow tool on the board. |
| boosterCompressor | A level-riding compressor with a treble-weighted detector and a saturator: drive levels, speed sets the release, gain is makeup. Moderate drive carries several dB of loudness. High drive flattens the crest. |
| boosterWide | Delays the side signal by about 1 ms. It combs each channel. Use `imaging` for width. |
| softClip | Acts only on peaks above its knee. After a large negative trim, it can stay inactive. |
| tube | Small drives change little. Large drives lower the level and the crest. |
| trimEq | A high-pass (12 dB per octave) and cuts at about 250 Hz and 3.5 kHz. |
| busCompressor | No makeup gain. A low threshold shrinks the loud-to-quiet spread. |
| maximizer | Drives a soft clip at -1 dBFS. It adds level with little change to the spectrum. |
| liftEq | Shelves at about 90 Hz and 12 kHz, 0-2 dB. |
| tape | Saturation, a head bump (lower at the higher speed), and a treble shelf. It darkens the top. |
| imaging | Mid/side width with the side high-passed below `monoBelow`. The mono sum does not change. |
| multiband | Three bands (split near 200 Hz and 3 kHz), each driven into a ceiling. Low adds weight, mid adds 400 Hz box, and high adds hat fizz. |
| limiter | True-peak brickwall at the end of the chain. |

## Output

- A table of the pedals: what each does on this mix.
- The recommended board as JSON (the shape of `board()` in
  `tools/song/mixjam.py`, trim included), saved as `board-roundN.json` in the
  review folder, and what each active pedal does in it.
- A table: recommended board, current board, and targets, for loudness, peak,
  crest, low-end shares, tonal indices, correlation, spread, section contrast,
  and the drum line.
- The more restrained and the bolder alternative, each with its settings and
  measurements.
- The mix problems that the board cannot fix, with numbers, bars, and tracks.
- The choices that need ears.
- The command that re-checks the board after the next mix change.
