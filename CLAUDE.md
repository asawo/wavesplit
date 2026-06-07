# Wavesplit

A cross-platform desktop practice app built with Tauri (Rust backend + Svelte frontend).

## What it does (current state)

- Accepts a YouTube URL, local audio file, or drag-and-drop file via an Add Track modal
- Downloads/converts audio to WAV (yt-dlp + ffmpeg) and probes duration with `ffprobe`
- Separates into stems: bass, drums, vocals, other (Demucs via Poetry)
- Manages a library of tracks with metadata (title, artist), rendered as a bordered table with Track / Artist / Duration / Status / Actions columns
- Exports stems + original audio to a user-chosen folder
- Full playback screen: synchronized 4-stem Web Audio engine, waveform display, per-stem mute/solo/volume, section loop
- Analysis stage is **stubbed out** — marked done immediately, no actual beat/note detection yet (TODO: MVP v3)

Primary use case: bass player practice with isolated stems.

## UI shell

- Persistent **Sidebar** on the left: Wavesplit script logo (with the app version from `getVersion()` underneath in JetBrains Mono), a single Library nav item, and an Add Track button in the footer that opens the modal.
- Right pane has a horizontal slide between the Library and Playback screens; clicking the sidebar's Library nav from Playback returns to the library.
- Add Track is a modal (`AddTrackModal.svelte`) with a YouTube URL pill + Add button on top, and a dashed drop zone below for either click-to-pick or OS drag-and-drop (wired through Tauri's `getCurrentWebview().onDragDropEvent`). Submitting the YouTube URL closes the modal immediately; the in-flight pipeline shows up as a row in the library.
- No global pipeline toast — per-row status dot in the library is the only in-flight indicator.

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
                    + ffprobe -show_entries format=duration → db.set_duration_ms (best-effort)
  → 2. stems        Demucs → stems/{bass,drums,vocals,other}.wav
  → 3. analysis     STUBBED — sets status_analysis=done immediately
```

Each stage updates the DB and emits a `pipeline` Tauri event `{ track_id, stage, status, message }`. `TrackList` subscribes to these events to drive the per-row Status column (pulsing yellow dot for any in-progress stage, green = ready, red = error).

## Key source files

| File | Purpose |
|------|---------|
| `core/src/lib.rs` | App entry point, AppState, command registration |
| `core/src/db.rs` | SQLite schema, migrations, all CRUD |
| `core/src/paths.rs` | Path helpers (track_dir, stems_dir, source_wav, etc.) |
| `core/src/commands.rs` | Tauri commands: add_track_youtube/local, export_stems, update_track_meta, open_folder, get_stem_paths |
| `core/src/pipeline/mod.rs` | Async pipeline orchestrator (download → stems → analysis) |
| `core/src/pipeline/download.rs` | yt-dlp / ffmpeg / ffprobe subprocess wrappers (incl. `probe_duration_ms`) |
| `core/src/pipeline/stems.rs` | Demucs subprocess, flattens output into stems/ |
| `core/src/pipeline/analysis.rs` | Analysis runner (currently unused; project_dir() is reused by stems.rs) |
| `ui/App.svelte` | Sidebar + content shell, library ↔ playback slide transition, AddTrackModal toggle, screen/selectedTrack state |
| `ui/lib/Sidebar.svelte` | Persistent left rail: brand + version, Library nav, Add Track button |
| `ui/lib/AddTrackModal.svelte` | Add Track modal: YouTube URL form + drag-drop / click drop zone |
| `ui/lib/importTrack.ts` | Shared `importLocalPath` + `pickAndImportLocal`; `ACCEPTED_AUDIO_EXTENSIONS` is the source of truth for file types |
| `ui/lib/TrackList.svelte` | Library table: search/sort toolbar, Music Library header, grid rows with Track / Artist / Duration / Status / Actions; inline edit, kebab Delete menu, export |
| `ui/lib/Playback.svelte` | Playback screen: Web Audio engine, waveforms, transport, stem mute/solo/volume, section loop, export |
| `ui/lib/playback.helpers.ts` | Pure functions: formatTime, hashStr, makeWaveformBars, extractWaveform, applyToggleSolo, computeMuted |
| `ui/lib/tracklist.helpers.ts` | Pure functions: fuzzy filter, sort fns, stageLabel/statusLabel, isReady/hasError |
| `python/analyze.py` | Python analysis script (not called yet) |
| `python/pyproject.toml` | Poetry project: librosa, numpy, demucs (torch 2.6.0) |

## Data model (SQLite)

Track columns: `id, title, artist, source_type, source_url, source_path, created_at, sort_order, duration_ms, status_download, status_stems, status_analysis, error_message, export_path`

Status values: `pending | done | error`

Migrations are additive via `.ok()` on `ALTER TABLE` in `db::open()`.

## Logging

Structured logging via `tracing` (configured in `lib.rs::run`):

- **File**: JSON format, daily rolling, written to `app_log_dir` (`~/Library/Logs/com.wavesplit.app/` on macOS), falls back to `<app_data_dir>/logs/`
- **stderr**: Human-readable format, visible during `just dev`
- **Default level**: `info` — override with `RUST_LOG` env var (e.g. `RUST_LOG=debug just dev`)
- The `NonBlockingGuard` is held in a `LogGuard` struct managed by Tauri (not leaked) so logs flush on shutdown

## Stack

- **Backend**: Rust (Tauri 2) — tokio async, rusqlite (bundled SQLite), uuid, chrono, tracing
- **Frontend**: Svelte 5 (runes), TypeScript, Vite, pnpm
- **External tools**: yt-dlp, ffmpeg, ffprobe (system install / bundled via Tauri sidecars); demucs (via Poetry venv in `python/`)
- **Fonts**: `@fontsource/oleo-script-swash-caps` (brand), `@fontsource/jetbrains-mono` (version + column headers), `@fontsource-variable/material-symbols-rounded`
- **Analysis**: Python 3.11+, Poetry, librosa, numpy, demucs

## Dev setup

```sh
brew install yt-dlp ffmpeg poetry
cd python && poetry install
pnpm install
just install-hooks  # install pre-commit hook (once per clone)
just dev            # or: pnpm run tauri dev
```

## Linting & CI

Run all checks before pushing:

```sh
just ci          # fmt-check + clippy + cargo test + vite build + svelte-check + prettier
just fix         # auto-format Rust + frontend (then re-run ci)
```

Individual commands:
- `just fmt-check` / `just fmt` — Rust formatting (check / fix)
- `just lint` — cargo clippy -D warnings
- `just test` — cargo test
- `just check-ui` — svelte-check (Svelte component errors)
- `just lint-ui` — Prettier formatting check
- `just build-ui` — Vite build (frontend only)

Frontend config: `.prettierrc` (Prettier), `jsconfig.json` (JS/Svelte type checking, excludes `dist/` and `*.test.js`).

CI runs on every push/PR (`ci.yml` for Rust, `ci-frontend.yml` for frontend). Both workflows mirror `just ci`.

## Important behaviours

- `analysis::project_dir()` walks 4 parent levels up from the binary to find `python/` — works in dev, will need revisiting for production packaging
- Demucs is invoked via `poetry run demucs` with `current_dir` set to the analysis project
- `list_tracks` returns newest-first (`ORDER BY sort_order DESC`)
- On startup, `mark_interrupted()` resets any `pending` pipeline stages to `error` so the UI can offer retry
- `pipeline::run` calls `download::probe_duration_ms` immediately after a successful download and persists via `db::set_duration_ms`. The probe runs inside `spawn_blocking`; failures (missing ffprobe, "N/A" duration, malformed output) are logged at `warn` and do not fail the pipeline
- `setup::remove_quarantine` treats `xattr -d com.apple.quarantine` exiting with `"No such xattr"` as a no-op success so a freshly extracted Demucs binary (no quarantine attr at all) doesn't break first-launch setup
- Playback screen uses Web Audio API: `AudioBufferSourceNode` per stem, `GainNode` per stem, RAF-driven playhead
- Audio files are loaded via `convertFileSrc` (Tauri asset protocol) + `fetch` + `decodeAudioData`
- `applyGains()` reads all `stemState` reactive values *before* any early returns so Svelte `$effect` tracks dependencies even before audio loads
- Track switching is handled by `$effect(() => { const id = track.id; if (id !== loadedTrackId) loadAudio() })` with a stale-load guard
- `assetProtocol` in `tauri.conf.json` requires the `protocol-asset` Cargo feature
- TrackList uses CSS custom props on `.tracks-table` (`--track-grid-columns`, `--track-grid-gap`) shared by `.tracks-header` and `.track` so the header and rows can't drift out of alignment
- `main` carries no padding; `TrackList` children (`.toolbar`, `.library-header`) own their own padding so `.tracks-table` is naturally full-bleed without negative margins
- AddTrackModal closes the modal synchronously before awaiting `addTrackYoutube` / `addTrackLocal`, so submit is effectively fire-and-forget from the user's perspective; errors are caught around the awaits and forwarded to `onAdded(null)`

## Scope

| Phase   | Status | Features |
|---------|--------|----------|
| MVP     | Done   | YouTube/local input, stem separation, library, export |
| MVP v2  | Done   | Playback engine, waveforms, stem mute/solo/volume, section loop |
| MVP v3  | Next   | Beat tracking, bass note display |
| Later   | —      | Chord detection |

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
