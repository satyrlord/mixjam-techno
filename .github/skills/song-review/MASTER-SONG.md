# Master the song

This file holds the body of step 8 of the [song-review skill](SKILL.md).

1. Launch the mastering engineer (`song-mastering-engineer`) as a fresh agent.
   Give it its inputs from [Role inputs](REFERENCE-INPUTS.md#role-inputs).
   Completion criterion: It returns a board JSON, a more restrained
   alternative, and a bolder alternative. It also returns the mix problems
   that the board cannot fix.
2. Accept or reject the board and each mix problem in the round log.
   Completion criterion: Each decision has a reason.
3. Fit the accepted board:
   `python -I tools/song/master.py PREMASTER BOARD.json OUT_DIR --song SONG
   --fit DB`.
   Completion criterion: The fitted `OUT_DIR/board.json` exists. The loud
   blocks of the premaster test meet the target, or your report states the
   difference.
4. Put the fitted board in the working song.
   Completion criterion: The song has the settings of `OUT_DIR/board.json`.
5. Set `MIXJAM_TECHNO_DATA_DIR` to a private folder for this render.
   Completion criterion: The variable names a private folder.
6. Render the working song to an existing `FINAL_DIR`:
   `.\target\release\mixjam-techno.exe render --mix SONG --output FINAL_DIR/master.wav`.
   Completion criterion: The fitted song made the final WAV.
7. Measure that render:
   `python -I tools/song/master.py PREMASTER OUT_DIR/board.json FINAL_DIR --song
   SONG --no-render`.
   Completion criterion: The round log has the result and each difference
   from the premaster test. Your report states the limits in
   [Measurement limits](REFERENCE-INPUTS.md#measurement-limits) that apply.
