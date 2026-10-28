# Chat 14: agent hearing steps 4 to 9

Done:
- Step 4: the reading, listen.py's summary word for word with ticks beside seconds; a `listen::Clock` gives onsets, segments and loop candidates exact ticks (c395aac). Checked line for line against listen.py on a test signal.
- Step 5: the sheet raster, listen.py's 3:1:1 layout, magma ramp and tab20 colours by sorted sound name (353e769).
- Step 6: `tickora_listen` `describe` (default true) and `sheet`, CLI `--listen` and `--sheet`, through `tickora_mcp::hearing::hear`; one replay pass shared with the WAV (c429997). `replay_audio` now takes only `to` and hands every tick to the sink, so a voice started before the range keeps its name; a range under 3 ticks keeps the sound list and reads why there is no analysis.
- Step 7: scenario checks `loudness_lufs`, `onsets`, `sound` with `starts`, one hearing per range per run (debug scenario tests 107 s to 46 s); arena.json 42 assertions (65a8ee5).
- Step 8: listen.py copied to `tools/audio_listen/`; `tests/tools/test_audio_listen.py` runs both on the arena WAV: every measure and the reading agree (80a85fb). Script index refreshed in claude-backup.
- Step 9: README hearing section, `docs/scenarios.md` (e33453e); ROADMAP 7.4 Done after CI run 40764387479 passed on Windows and Linux.

Flags opened: none.
Flags still open: Felipe judges the arena's spawn and hit balance by ear; Corpofobia's reply to the hearing proposal unread; K-weighting gap against Keel.
