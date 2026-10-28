# How other tools let an agent hear

Prior art for ROADMAP 7.4, agent hearing. Sources are directory listings and READMEs, read once; tool schemas were not checked.

## Unreal

- Audio Synesthesia, non-real-time analyzers run in the editor on a sound, results read from a file at runtime: LoudnessNRT (perceptual loudness over time, frequency range and noise floor settings), ConstantQNRT (band strengths in perceptual bands; analysis period, starting frequency, bands, bands per octave, downmix to mono), OnsetNRT (onset events with timestamp and strength; granularity and sensitivity). Epic, "Audio Synesthesia in Unreal Engine".
- Audio Insights, Sounds tab: every playing sound by category with its runtime data. `au.Debug.Sounds` prints sound information on the viewports. Epic, "Audio Insights in Unreal Engine", "Audio Console Commands".

## Audio MCP servers

- music-perception-mcp: deterministic tools for loudness, true peak, tempo, key, spectral balance and clipping with first timestamps; subjective judgement kept in a separate, non-deterministic tool backed by an audio model.
- Beatlyzer: librosa beat tracking; three tools over stdio, full JSON, a short prose description, and an annotated PNG for the model to view.
- Pro Tools MCP: bounces a region and returns waveform, spectrogram, peak and RMS views and onsets; its docs say models like Claude cannot listen, so it describes audio visually and in text.
- Audio Analyzer (juzzydee): pure Rust; spectral, harmonic, rhythm, timbre and loudness numbers.
- Carla MCP: real-time spectrum analysis among its tools.

## Game engine MCP servers

Godot MCP servers (Farraskuy, keeveeg, Godot MCP Pro) expose audio state only: bus layout, effects, players, playing streams. None found renders and analyses the output. No Unity MCP audio tool turned up.

## Takeaways for Tickora

- The common shape is deterministic numbers plus a short prose reading plus an optional image, which matches Corpofobia's listen.py (text summary, JSON, PNG sheet).
- Keeping exact measures apart from any model-based judgement matches Tickora's split between hashed state and derived output.
- No game engine MCP found hears its own rendered mix; Tickora would be first among those checked.
