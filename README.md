# Wavesplit

<img width="600" alt="wavesplit screenshot" src="https://github.com/user-attachments/assets/1db258ee-9cfd-4a82-aee4-e63c59664bd6" />

A desktop app for musicians to practice with isolated stems. Give it a YouTube URL or a local audio file - it separates the audio into bass, drums, vocals, and other, and lets you export the stems for use in a DAW or practice session.

Built with Tauri (Rust + Svelte).

---

## Features

- Add tracks from a YouTube URL or a local audio file
- Stem separation via [Demucs](https://github.com/facebookresearch/demucs) (bass, drums, vocals, other)
- Export stems + original audio to any folder
- Library with search and sort (by newest, oldest, title, or artist)
- Track metadata editing (title and artist)
- Real-time progress events during pipeline stages

---

## Installation (macOS)

Download the latest `.dmg` from the [Releases](https://github.com/asawo/wavesplit/releases) page, open it, and drag Wavesplit to your Applications folder.

### "Apple could not verify Wavesplit is free of malware"

Wavesplit is not notarized with Apple (that requires a $99/year developer account). macOS will block the first launch, but it's safe to open.

Open **System Settings → Privacy & Security**, scroll down to the "Security" section, and click **Open Anyway** next to the Wavesplit entry.

You only need to do this once.

---

## Development

### Prerequisites

- [Rust](https://rustup.rs/)
- [Node.js](https://nodejs.org/) + [pnpm](https://pnpm.io/)
- [yt-dlp](https://github.com/yt-dlp/yt-dlp) — for YouTube downloads
- [ffmpeg](https://ffmpeg.org/) — for audio conversion
- [Poetry](https://python-poetry.org/) — for the Python analysis environment
- Python 3.11+ (via [asdf](https://asdf-vm.com/) or system install)

```sh
# Install Python dependencies
cd python
poetry install
```

```sh
# Install JS dependencies
pnpm install

# Start dev server with hot reload
just dev
# or: pnpm run tauri dev
```

Other commands:

```sh
just build       # Build release binary + installer
just check       # Check Rust without building
just test        # Run Rust tests
just fmt         # Format Rust code
just lint        # Clippy (warnings as errors)
just open-data   # Open app data directory (macOS)
just reset-data  # Wipe all app data and tracks (destructive)
```

---

## Architecture

```
UI (Svelte / Tauri webview)
  → Rust backend (Tauri commands)
    → yt-dlp       (YouTube download)
    → ffmpeg        (audio conversion to WAV)
    → Demucs        (stem separation, via Poetry)
    → SQLite        (track metadata, bundled via rusqlite)
```

Each pipeline stage (download → stems → analysis) updates the database and emits a `pipeline` event to the frontend for live progress display.

Track data is stored in:
- `~/Library/Application Support/com.wavesplit.app/` (macOS)

---

## Roadmap

| Phase   | Features |
|---------|----------|
| MVP     | YouTube/local input, stem separation, library, export |
| MVP v2  | Playback engine, beat tracking, bass note display |
| Later   | Chord detection, stem mute/solo, loop sections, waveform view |

---

## Contributing

**Commit format:** `<type>: <short description>` (lowercase, no period)

| Type | When to use |
|------|-------------|
| `add` | New feature, file, or capability |
| `fix` | Bug fix |
| `chore` | Maintenance, releases, tooling |
| `refactor` | Code restructure with no behaviour change |
| `docs` | Documentation only |

**Branch naming:** `<type>/<short-description>` — e.g. `add/playback-engine`, `fix/stem-export-path`

**PRs:**
- Title mirrors commit format: `add: playback engine`
- One logical change per PR
- Description explains *why*, not just *what*
