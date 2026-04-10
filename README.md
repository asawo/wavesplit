# Wavesplit

<img width="600" alt="wavesplit screenshot" src="https://github.com/user-attachments/assets/1db258ee-9cfd-4a82-aee4-e63c59664bd6" />

A desktop app for musicians to practice with isolated stems. Give it a YouTube URL or a local audio file — it separates the audio into bass, drums, vocals, and other so you can play along, mute parts, or export them to a DAW.

Built with Tauri (Rust + Svelte).

---

## Features

- Add tracks from a YouTube URL or a local audio file via [yt-dlp](https://github.com/yt-dlp/yt-dlp)
- Stem separation via [Demucs](https://github.com/facebookresearch/demucs) (bass, drums, vocals, other)
- Full playback screen with synchronized 4-stem audio engine
- Per-stem mute, solo, and volume control
- Export stems + original audio to any folder
- Library with search and sort (by newest, oldest, title, or artist)
- Track metadata editing (title and artist)

---

## How to use

### 1. First launch — download Demucs

Wavesplit uses [Demucs](https://github.com/facebookresearch/demucs) to separate audio into stems. On first launch, a screen will prompt you to download the audio separation engine. Click **Download** and wait for the one-time download to complete.

### 2. Add a track

- **YouTube:** paste a YouTube URL into the input field and press Enter or click **Add**
- **Local file:** click **Open file** and select an audio file (MP3, WAV, FLAC, etc.)

The track appears in the library immediately. Three pipeline stages run in sequence:

| Stage | What happens |
|-------|-------------|
| Download | Fetches the audio and converts it to WAV |
| Stems | Demucs separates it into bass, drums, vocals, other |
| Analysis | Finalizes the track |

Progress is shown live in the track row. Stem separation typically takes 1–5 minutes depending on your machine.

### 3. Play a track

Once a track shows **Ready**, click anywhere on its row to open the playback screen.

**Transport controls:**

| Button | Action |
|--------|--------|
| ‹ | Jump to start |
| ‹‹ | Rewind 10 seconds |
| ▶ / ⏸ | Play / Pause |
| ›› | Forward 10 seconds |
| › | Jump to end |

Click anywhere on the waveform to seek to that position.

### 4. Mix the stems

Each stem row has three controls:

- **M** — mute that stem
- **S** — solo that stem (exclusive: only the soloed stem plays; click again to clear)
- **Volume slider** — adjust the level independently

### 5. Edit track metadata

In the library, click the track title or artist name to edit it inline. Press Enter or click away to save.

### 6. Export stems

Click **↓ Export stems** on any ready track (in the library or the playback screen) to copy the separated WAV files to a folder of your choice. The exported folder contains:

- `vocals.wav`
- `drums.wav`
- `bass.wav`
- `other.wav`
- `source.wav` (the original converted audio)

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

| Phase   | Status | Features |
|---------|--------|----------|
| MVP     | ✅ | YouTube/local input, stem separation, library, export |
| MVP v2  | ✅ | Playback engine, waveforms, stem mute/solo/volume |
| V1  | ✏️ | Beat tracking, key display |

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
