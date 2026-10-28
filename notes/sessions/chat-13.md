# Chat 13: HPL2 check, agent hearing design and steps 1 to 3

Done:
- HPL2 measured as an alternative to Tickora (`docs/research/hpl2-fork.md`, cf9cc6b, 3fff5a4): it does not build as shipped (FBX SDK 2012), Newton 2.00 float physics blocks cross-platform replay hashes, windowed OpenGL only; Felipe kept Tickora and logged an HPL2 MCP bridge as an unscheduled side project.
- Prior art survey (`docs/research/agent-hearing.md`, cabbde5): Unreal's Audio Synesthesia and Audio Insights; audio MCP servers return numbers, prose and images; no game engine MCP found hears its own mix.
- Design `docs/design/phase-7-hearing.md`, approved by Felipe after choices made one at a time: sound list plus mix analysis; Rust port with no new dependency; numbers, reading and spectrogram; every listen.py measure; three scenario checks. Memory figure for the 36,000-tick cap corrected to about 1.6 GB; Felipe kept the cap (e85b2f0).
- Step 1: voice start, end and drop events on `Block`, `Mixer::graph`, `Host::replay_audio` and a free `host::write_wav` (9d9ce34).
- Step 2: `tickora_audio::listen` FFT, biquads, K-weighting, block and integrated loudness, true peak, STFT, bands, centroid, flatness (0322a52). Finding: listen.py reads -20.25 LUFS for a 1 kHz sine at -20 dBFS, about 0.25 dB below strict BS.1770; kept for agreement with listen.py.
- Step 3: onsets, tones (medfilt, find_peaks), stereo, voice likeness (Butterworth band-pass, filtfilt, welch, checked against scipy coefficients), seams and loop search, timeline, `analyse` (2747268).
- Sent the Corpofobia session a proposal to adopt the same hearing (Felipe's request).
- Outside Tickora: the 8 Hostinger MCP servers moved from user scope to local scope in Cuña-Cutter, Minga3D, Corpofobia and the Brown Marco Studios folder (projects whose sessions called them); their running processes were stopped.

Flags opened: K-weighting gap against Keel; Corpofobia's reply to the hearing proposal unread.
Flags still open: Felipe judges the arena's spawn and hit balance by ear.
