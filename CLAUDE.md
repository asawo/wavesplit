# Wavesplit

A cross-platform desktop practice app built with Tauri (Rust backend + Svelte frontend).

## What it does (current state)

- Accepts a YouTube URL or local audio file
- Downloads/converts audio to WAV (yt-dlp + ffmpeg)
- Separates into stems: bass, drums, vocals, other (Demucs via Poetry)
- Manages a library of tracks with metadata (title, artist)
- Exports stems + original audio to a user-chosen folder
- Analysis stage is **stubbed out** — marked done immediately, no actual beat/note detection yet (TODO: MVP v2)

Primary use case: bass player practice with isolated stems.

## Architecture

```
UI (Svelte 5 / Tauri webview)
  → Rust backend (Tauri commands + async pipeline)
    → yt-dlp        (YouTube download)
    → ffmpeg         (WAV conversion)
    → Demucs         (stem separation, via `poetry run demucs`)
    → SQLite         (track metadata, bundled via rusqlite)
```

## Pipeline stages

```
add_track(source)
  → 1. download     yt-dlp or ffmpeg → source.wav
  → 2. stems        Demucs → stems/{bass,drums,vocals,other}.wav
  → 3. analysis     STUBBED — sets status_analysis=done immediately
```

Each stage updates the DB and emits a `pipeline` Tauri event `{ track_id, stage, status, message }` for frontend progress display.

## Key source files

| File | Purpose |
|------|---------|
| `src-tauri/src/lib.rs` | App entry point, AppState, command registration |
| `src-tauri/src/db.rs` | SQLite schema, migrations, all CRUD |
| `src-tauri/src/paths.rs` | Path helpers (track_dir, stems_dir, source_wav, etc.) |
| `src-tauri/src/commands.rs` | Tauri commands: add_track_youtube/local, export_stems, update_track_meta, open_folder |
| `src-tauri/src/pipeline/mod.rs` | Async pipeline orchestrator (download → stems → analysis) |
| `src-tauri/src/pipeline/download.rs` | yt-dlp and ffmpeg subprocess wrappers |
| `src-tauri/src/pipeline/stems.rs` | Demucs subprocess, flattens output into stems/ |
| `src-tauri/src/pipeline/analysis.rs` | Analysis runner (currently unused; project_dir() is reused by stems.rs) |
| `src/App.svelte` | Root layout, CSS variables, section structure |
| `src/lib/AddTrack.svelte` | YouTube URL input + local file picker |
| `src/lib/TrackList.svelte` | Library: filter, sort, inline edit, progress, export, delete |
| `python/analyze.py` | Python analysis script (not called yet) |
| `python/pyproject.toml` | Poetry project: librosa, numpy, demucs (torch 2.6.0) |

## Data model (SQLite)

Track columns: `id, title, artist, source_type, source_url, source_path, created_at, sort_order, duration_ms, status_download, status_stems, status_analysis, error_message, export_path`

Status values: `pending | done | error`

Migrations are additive via `.ok()` on `ALTER TABLE` in `db::open()`.

## Stack

- **Backend**: Rust (Tauri 2) — tokio async, rusqlite (bundled SQLite), uuid, chrono
- **Frontend**: Svelte 5 (runes), Vite, pnpm
- **External tools**: yt-dlp, ffmpeg (system install); demucs (via Poetry venv in `python/`)
- **Analysis**: Python 3.11+, Poetry, librosa, numpy, demucs

## Dev setup

```sh
brew install yt-dlp ffmpeg poetry
cd python && poetry install
pnpm install
just dev        # or: pnpm run tauri dev
```

## Important behaviours

- `analysis::project_dir()` walks 4 parent levels up from the binary to find `python/` — works in dev, will need revisiting for production packaging
- Demucs is invoked via `poetry run demucs` with `current_dir` set to the analysis project
- `list_tracks` returns newest-first (`ORDER BY sort_order DESC`)
- On startup, `incomplete_tracks()` logs any tracks with unfinished pipeline state (not auto-retried)

## Scope

| Phase   | Status | Features |
|---------|--------|----------|
| MVP     | Done   | YouTube/local input, stem separation, library, export |
| MVP v2  | Next   | Playback engine, beat tracking, bass note display |
| Later   | —      | Chords, stem mute/solo, loop sections, waveform view |

## Commit & PR conventions

**Commit format:** `<type>: <short description>` (lowercase, no period)

| Type | When to use |
|------|-------------|
| `add` | New feature, file, or capability |
| `fix` | Bug fix |
| `chore` | Maintenance, releases, tooling |
| `refactor` | Code restructure with no behaviour change |
| `docs` | Documentation only |

**Branch naming:** `<type>/<short-description>` — e.g. `add/playback-engine`, `fix/stem-export-path`, `refactor/pipeline-stages`

**PR conventions:**
- Title mirrors commit format: `add: playback engine`
- One logical change per PR
- PR description explains *why*, not just *what*

## Key constraints

- Bass accuracy is the top priority
- No real-time ML during playback — precompute everything
- All analysis precomputed and stored as JSON
- Beat/bar grid optimized for common time (4/4)
