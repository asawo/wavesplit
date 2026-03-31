Updated project brief (v2)

Build a cross-platform desktop practice app using Tauri (Rust backend + webview UI) that:
	•	accepts a YouTube URL or local audio file
	•	extracts and separates audio into stems
	•	analyzes musical structure (notes, chords, beats, bars)
	•	plays stems with synchronized visual overlays for practice

⸻

Core goal

Provide a practice-focused playback experience, especially for bass players, with:
	•	isolated stems
	•	synchronized note/chord display per stem
	•	beat/bar grid derived from drums

⸻

Requirements

Input
	•	YouTube URL (via yt-dlp)
	•	Local audio file

Audio pipeline
	•	Extract audio → convert to WAV
	•	Separate stems using Demucs:
	•	bass
	•	drums
	•	vocals
	•	other

Analysis outputs
	•	Drum stem
	•	beat tracking (tempo + beats)
	•	downbeat detection
	•	bar segmentation
	•	All harmonic stems (bass, vocals, other)
	•	note detection (pitch over time)
	•	chord estimation (per beat or per bar)

UI
	•	Desktop app (Tauri)
	•	Features:
	•	playback controls
	•	stem mute/solo
	•	waveform or timeline view
	•	current:
	•	note
	•	chord
	•	bar / beat position
	•	Timeline should be synchronized to playback time

⸻

Key design decision

All analysis is precomputed, not done live.

separate → analyze → store → playback + sync

This ensures:
	•	stable results
	•	low CPU usage
	•	accurate UI synchronization

⸻

Analysis model

Step 1: timing grid (drums)
	•	extract beats and tempo
	•	detect downbeats
	•	construct bar structure:

{ "bar": 12, "beat": 3, "time": 45.23 }


⸻

Step 2: harmonic analysis (per stem)

For each stem:
	•	align analysis to beat/bar grid
	•	generate:

{
  "time": 45.23,
  "stem": "bass",
  "note": "A2",
  "chord": "Am"
}

Important:
	•	bass → strongest and most reliable
	•	vocals/other → best-effort
	•	drums → timing only (no chord)

⸻

Data model

Two main outputs:

1. Timing grid

{
  "tempo": 120,
  "beats": [...],
  "bars": [...]
}

2. Stem annotations

{
  "bass": [...],
  "vocals": [...],
  "other": [...]
}


⸻

Backend (Rust)

Responsibilities:
	•	orchestrate pipeline
	•	run external tools:
	•	yt-dlp
	•	ffmpeg
	•	demucs
	•	run analysis step (can call Python initially)
	•	manage file system + temp directories
	•	emit progress events to UI
	•	output:
	•	stems (audio)
	•	JSON analysis files

⸻

Frontend (Tauri UI)

Responsibilities:
	•	playback engine (Web Audio API)
	•	sync UI with current playback time
	•	display:
	•	current note/chord per stem
	•	current bar/beat
	•	allow:
	•	stem mute/solo
	•	seeking
	•	loop sections

⸻

Packaging
	•	Use Tauri build system
	•	Bundle:
	•	Rust binary
	•	frontend assets
	•	external tools as sidecars (yt-dlp, ffmpeg, demucs)
	•	Produce platform-specific installers

⸻

Architecture

UI (Tauri webview)
  → Rust backend
  → yt-dlp / ffmpeg / demucs
  → stems (audio)
  → analysis (JSON)

Playback:
  audio time → lookup JSON → update UI


⸻

Key constraints
	•	prioritize bass accuracy and usability
	•	chord detection is approximate
	•	beat/bar grid is best-effort (optimized for common time)
	•	avoid real-time ML during playback

⸻

Scope guidance (important)

MVP
	•	YouTube/local input
	•	stem separation
	•	playback + sync
MVP v2
	•	bass note display
	•	basic beat tracking
Later
	•	chords
	•	multi-stem analysis
	•	improved UI/visualizations
	•	editing/looping tools

