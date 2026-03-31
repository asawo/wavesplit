# Wavesplit

A cross-platform desktop practice app built with Tauri (Rust backend + webview UI).

## What it does

- Accepts a YouTube URL or local audio file
- Extracts and separates audio into stems (bass, drums, vocals, other) via Demucs
- Analyzes musical structure: notes, chords, beats, bars (all **precomputed**, not live)
- Plays stems with synchronized visual overlays for practice

Primary use case: bass player practice with isolated stems and synchronized note/chord display.

## Architecture

```
UI (Tauri webview)
  → Rust backend
    → yt-dlp / ffmpeg / demucs  (external sidecars)
    → stems (audio files)
    → analysis (JSON files)

Playback: audio time → lookup JSON → update UI
```

## Pipeline

1. Input: YouTube URL (via yt-dlp) or local audio file
2. Extract audio → convert to WAV (ffmpeg)
3. Separate stems: bass, drums, vocals, other (Demucs)
4. Analyze:
   - Drums → beat tracking, downbeat detection, bar segmentation
   - Bass/vocals/other → note detection, chord estimation (aligned to beat grid)
5. Store results as JSON
6. UI plays audio + syncs with precomputed JSON

## Data model

**Timing grid** (from drums):
```json
{ "tempo": 120, "beats": [...], "bars": [{ "bar": 12, "beat": 3, "time": 45.23 }] }
```

**Stem annotations**:
```json
{ "bass": [{ "time": 45.23, "stem": "bass", "note": "A2", "chord": "Am" }], "vocals": [...], "other": [...] }
```

## Stack

- **Backend**: Rust (Tauri) — orchestrates pipeline, runs external tools, emits progress events, manages filesystem
- **Frontend**: Web (Tauri webview) — Web Audio API for playback, sync UI to current time, stem mute/solo, seeking, loop sections
- **External tools** (bundled as sidecars): yt-dlp, ffmpeg, demucs
- **Analysis**: Python (initial), called from Rust

## Scope

| Phase    | Features |
|----------|----------|
| MVP      | YouTube/local input, stem separation, playback + sync |
| MVP v2   | Bass note display, basic beat tracking |
| Later    | Chords, multi-stem analysis, improved UI, editing/looping |

## Key constraints

- Bass accuracy is the top priority
- Chord detection is approximate / best-effort
- Beat/bar grid optimized for common time (4/4)
- No real-time ML during playback — precompute everything
- Packaging: Tauri build system, platform-specific installers
