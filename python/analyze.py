#!/usr/bin/env python3
"""
analyze.py <stems_dir> <analysis_dir>

Reads stems from <stems_dir> and writes:
  <analysis_dir>/timing.json   — tempo, beats, bars
  <analysis_dir>/stems.json    — per-stem note/chord annotations aligned to beat grid
"""

import sys
import json
from pathlib import Path

try:
    import librosa
    import numpy as np
except ImportError as e:
    print(f"Missing dependency: {e}\nRun: pip install librosa numpy", file=sys.stderr)
    sys.exit(1)


def analyze_timing(drums_path: Path) -> dict:
    """Extract beat grid from the drum stem."""
    y, sr = librosa.load(str(drums_path), mono=True)
    tempo, beat_frames = librosa.beat.beat_track(y=y, sr=sr, units="frames")
    beat_times = librosa.frames_to_time(beat_frames, sr=sr).tolist()

    # Detect downbeats (every 4 beats = bar in 4/4)
    bars = []
    for bar_idx, i in enumerate(range(0, len(beat_times), 4)):
        bars.append({
            "bar": bar_idx + 1,
            "beat": 1,
            "time": beat_times[i],
        })

    beats = [{"time": t, "beat": (i % 4) + 1} for i, t in enumerate(beat_times)]

    return {
        "tempo": float(np.atleast_1d(tempo)[0]),
        "beats": beats,
        "bars": bars,
    }


def analyze_stem_notes(stem_path: Path, beat_times: list[float]) -> list[dict]:
    """Extract dominant pitch per beat for a harmonic stem."""
    y, sr = librosa.load(str(stem_path), mono=True)
    annotations = []

    for i, beat_time in enumerate(beat_times):
        next_time = beat_times[i + 1] if i + 1 < len(beat_times) else beat_time + 0.5
        start = int(beat_time * sr)
        end = int(next_time * sr)
        segment = y[start:end]

        note = None
        if len(segment) > 0 and np.max(np.abs(segment)) > 0.01:
            # Estimate fundamental frequency via pyin
            f0, voiced_flag, _ = librosa.pyin(
                segment,
                fmin=librosa.note_to_hz("C1"),
                fmax=librosa.note_to_hz("C7"),
                sr=sr,
            )
            voiced = f0[voiced_flag] if voiced_flag is not None else np.array([])
            if len(voiced) > 0:
                median_f0 = float(np.median(voiced))
                note = librosa.hz_to_note(median_f0)

        annotations.append({"time": beat_time, "note": note})

    return annotations


def main():
    if len(sys.argv) != 3:
        print("Usage: analyze.py <stems_dir> <analysis_dir>", file=sys.stderr)
        sys.exit(1)

    stems_dir = Path(sys.argv[1])
    analysis_dir = Path(sys.argv[2])
    analysis_dir.mkdir(parents=True, exist_ok=True)

    drums_path = stems_dir / "drums.wav"
    if not drums_path.exists():
        print(f"drums.wav not found in {stems_dir}", file=sys.stderr)
        sys.exit(1)

    print("Analyzing timing from drums...", flush=True)
    timing = analyze_timing(drums_path)
    (analysis_dir / "timing.json").write_text(json.dumps(timing, indent=2))
    print(f"  tempo={timing['tempo']:.1f} bpm, {len(timing['beats'])} beats, {len(timing['bars'])} bars")

    beat_times = [b["time"] for b in timing["beats"]]

    stem_annotations = {}
    for stem_name in ("bass", "vocals", "other"):
        stem_path = stems_dir / f"{stem_name}.wav"
        if not stem_path.exists():
            print(f"  skipping {stem_name} (not found)", flush=True)
            continue
        print(f"Analyzing {stem_name}...", flush=True)
        stem_annotations[stem_name] = analyze_stem_notes(stem_path, beat_times)
        print(f"  done ({len(stem_annotations[stem_name])} beats annotated)")

    (analysis_dir / "stems.json").write_text(json.dumps(stem_annotations, indent=2))
    print("Analysis complete.")


if __name__ == "__main__":
    main()
