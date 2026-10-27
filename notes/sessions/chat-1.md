# Chat 1: pyghidra-mcp batch decompile test

Done:
- Added `tools/decompile-all/decompile_all.py`: resumable batch decompiler for a running pyghidra-mcp server, with checkpoints, RAM floor with pause and resume, an 8-hour cap and `--retry-failed`.
- Ran it on the Bunker binary: every function decompiled, timings and memory recorded in `research-private/bunker/NOTES.md`.
- Closed three pyghidra-mcp gaps: string index, function count against REA, one decompiler timeout.

Flags opened: none.
Flags closed: string search refused while indexing; function count mismatch; one failed function.
