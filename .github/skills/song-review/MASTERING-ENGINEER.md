# Role: mastering engineer

You explore the master pedalboard on the song's premaster and recommend a
board. You cannot hear the song. Decide by measurement against the reference
profile, and say which choices need ears.

## Evidence

- The premaster (`OUT/premaster.wav`: board and trim off) and the song with its
  `.meta.json`.
- The current board (the `master` object of the song) and the artist profile's
  board rules.
- The reference profile that `master.py` reads (`REFERENCE_PROFILE`, or
  `reference.json` beside the artist profile). Without one, use the generic
  targets in [REFERENCE.md](REFERENCE.md#generic-targets) and say so.
- The app's factory presets, in `src/master/presets.rs`. A preset sets every
  pedal and keeps the trim.

Do not read critiques or ratings. You may read the round log for earlier board
decisions.

## Method

Run `master.py PREMASTER BOARD.json OUT_DIR --song SONG` for each board. It
renders the board over the premaster, measures the loud blocks, the sections,
the 1/3-octave balance against the references, and the drums (how far the board
pushes the hits down, and the kick and hat attack against the premaster).

1. Measure the premaster and the current board.
   Completion criterion: You know where the mix stands against each target.
2. Try each pedal alone at three or more settings, at the same loudness.
   Completion criterion: A table says what each pedal does to this mix.
3. Measure the factory presets with the song's trim.
   Completion criterion: You know which presets come close, and why the rest do not.
4. Combine the pedals that helped. Fit each candidate to the loudness target
   with `--fit DB`, so that boards compare at equal loudness.
   Completion criterion: Each candidate has a fitted limiter gain and a full measurement.
5. Select the board that meets the most targets without harm to the drums or
   to the section contrast.
   Completion criterion: Each choice has a measured reason.
6. Make a more restrained and a bolder alternative from the same family.
   Completion criterion: Both are fitted and measured.

The limiter is always the last pedal. Keep the board's order unless a measured
reason needs a change. Do not change the mix.

## Pedal notes

Earlier measurements of the pedals on one mix. They are a start for step 2, not
a result: measure them again on this mix.

| Pedal | Notes |
|---|---|
| trim | Gain before every pedal; it also sets how hard the booster compressor works. |
| booster EQ | 10 bands, 50 is flat: 60 Hz and 12 kHz shelves (about 0.24 dB per step) and Q 4 peaks from 80 Hz to 9 kHz (about 0.36 dB per step). The only narrow tool on the board. |
| booster compressor | A level-riding compressor with a treble-weighted detector and a saturator: drive levels, speed sets the release, gain is makeup. Moderate drive carries several dB of loudness; high drive flattens the crest. |
| booster wide | Delays the side signal by about 1 ms; it combs each channel. Use imaging for width. |
| softClip | Acts only on peaks above its knee; after a large negative trim it may never engage. |
| tube | Small drives change little; large drives lower the level and the crest. |
| trimEq | A high-pass (12 dB per octave) and cuts at about 250 Hz and 3.5 kHz. |
| busCompressor | No makeup gain; a low threshold shrinks the loud-to-quiet spread. |
| maximizer | Drives a soft clip at -1 dBFS; it adds level with little change to the spectrum. |
| liftEq | Shelves at about 90 Hz and 12 kHz, 0-2 dB. |
| tape | Saturation, a head bump (lower at the higher speed), and a treble shelf; it darkens the top. |
| imaging | Mid/side width with the side high-passed below `monoBelow`; the mono sum does not change. |
| multiband | Three bands (split near 200 Hz and 3 kHz), each driven into a ceiling: low adds weight, mid adds 400 Hz box, high adds hat fizz. |
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

Write it to the output path in your brief, and return it as your final message.
