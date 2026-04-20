#!/usr/bin/env python3
"""
analyze.py <stems_dir> <source_wav> <analysis_dir>

Detects beat grid and chords from the source audio using beat_this + Essentia.
Writes <analysis_dir>/analysis.json.
"""

import sys
import json
from pathlib import Path

try:
    import essentia.standard as es
    import numpy as np
    from beat_this.inference import File2Beats
except ImportError as e:
    print(f"Missing dependency: {e}\nRun: cd python && poetry install", file=sys.stderr)
    sys.exit(1)

SAMPLE_RATE = 44100
FRAME_SIZE = 4096
HOP_SIZE = 2048
MIN_CHORD_STRENGTH = 0.15

# Loaded once; downloads ~50 MB checkpoint on first use (cached in torch hub dir).
_file2beats = File2Beats(checkpoint_path="final0", device="cpu", dbn=False)


def detect_beats(source_wav):
    """
    Return (bpm, beat_times, downbeat_times) using beat_this.
    beat_times and downbeat_times are numpy arrays of seconds; downbeats is a
    subset of beats aligned to bar starts.
    """
    beats, downbeats = _file2beats(str(source_wav))
    beats = np.asarray(beats)
    downbeats = np.asarray(downbeats)
    bpm = 60.0 / float(np.median(np.diff(beats))) if len(beats) >= 2 else 120.0
    return float(bpm), beats, downbeats


def load_chord_audio(stems_dir):
    """
    Mix bass + other + vocals (everything except drums) for chord detection.
    other.wav is required; bass and vocals are added when present.
    """
    stems_dir = Path(stems_dir)
    if not (stems_dir / "other.wav").exists():
        raise FileNotFoundError(
            f"other.wav not found in {stems_dir} — stems stage may not have completed"
        )

    arrays = [
        es.MonoLoader(filename=str(stems_dir / f"{name}.wav"), sampleRate=SAMPLE_RATE)()
        for name in ("other", "bass", "vocals")
        if (stems_dir / f"{name}.wav").exists()
    ]

    if len(arrays) == 1:
        return arrays[0]

    max_len = max(len(a) for a in arrays)
    mixed = np.zeros(max_len, dtype=np.float32)
    for a in arrays:
        mixed[: len(a)] += a
    peak = np.max(np.abs(mixed))
    if peak > 1e-6:
        mixed /= peak
    return mixed


def compute_hpcps(stems_dir):
    """
    Load the chord stem mix, apply EqualLoudness, return HPCP matrix (N, 12).

    Uses harmonics=8 — essential for major/minor chord quality detection.
    HOP_SIZE must match the hopSize passed to ChordsDetectionBeats for correct
    frame-to-time alignment.
    """
    audio = es.EqualLoudness()(load_chord_audio(stems_dir))

    windowing = es.Windowing(type="blackmanharris62")
    spectrum = es.Spectrum()
    spectral_peaks = es.SpectralPeaks(
        magnitudeThreshold=0.00001,
        maxPeaks=60,
        maxFrequency=3500,
        minFrequency=20,
        orderBy="magnitude",
    )
    hpcp_algo = es.HPCP(
        size=12,
        referenceFrequency=440.0,
        bandPreset=False,
        minFrequency=20.0,
        maxFrequency=3500.0,
        weightType="squaredCosine",
        nonLinear=True,
        windowSize=4.0 / 3.0,
        harmonics=8,
    )

    hpcps = [
        hpcp_algo(*spectral_peaks(spectrum(windowing(frame))))
        for frame in es.FrameGenerator(audio, frameSize=FRAME_SIZE, hopSize=HOP_SIZE, startFromZero=True)
    ]
    return np.array(hpcps) if hpcps else np.zeros((0, 12))


def detect_chords(hpcps, beat_times):
    """
    Use ChordsDetectionBeats to get one chord label per beat segment.
    Returns (chords: list[str], strengths: list[float]) aligned to beat_times.
    """
    ticks = np.array([float(bt) for bt in beat_times], dtype=np.float32)
    chords, strengths = es.ChordsDetectionBeats(hopSize=HOP_SIZE)(hpcps, ticks)
    return list(chords), [float(s) for s in strengths]


def detect_key(audio):
    """Return global key string like 'A minor'."""
    key, scale, _ = es.KeyExtractor()(audio)
    return f"{key} {scale}"


def main():
    if len(sys.argv) != 4:
        print("Usage: analyze.py <stems_dir> <source_wav> <analysis_dir>", file=sys.stderr)
        sys.exit(1)

    stems_dir = Path(sys.argv[1])
    source_wav = Path(sys.argv[2])
    analysis_dir = Path(sys.argv[3])
    analysis_dir.mkdir(parents=True, exist_ok=True)

    if not source_wav.exists():
        print(f"source.wav not found: {source_wav}", file=sys.stderr)
        sys.exit(1)

    print("Loading audio...", flush=True)
    audio = es.MonoLoader(filename=str(source_wav), sampleRate=SAMPLE_RATE)()

    print("Detecting beats and downbeats...", flush=True)
    bpm, beat_times, downbeat_times = detect_beats(source_wav)
    print(f"  {bpm:.1f} bpm, {len(beat_times)} beats, {len(downbeat_times)} downbeats", flush=True)

    print("Computing HPCP frames...", flush=True)
    hpcps = compute_hpcps(stems_dir)
    print(f"  {len(hpcps)} HPCP frames", flush=True)

    print("Detecting chords...", flush=True)
    chords, strengths = detect_chords(hpcps, beat_times)
    print(f"  {len(chords)} beat chords", flush=True)

    print("Detecting key...", flush=True)
    key = detect_key(audio)
    print(f"  key: {key}", flush=True)

    # Map each downbeat time to its index in beat_times
    downbeat_indices = {int(np.argmin(np.abs(beat_times - db))) for db in downbeat_times}

    beat_num = 1
    beats_list = []
    for i, bt in enumerate(beat_times):
        if i in downbeat_indices:
            beat_num = 1
        beats_list.append({
            "time": float(bt),
            "beat": beat_num,
            "chord": chords[i] if i < len(chords) and strengths[i] >= MIN_CHORD_STRENGTH else "—",
            "chord_strength": strengths[i] if i < len(strengths) else 0.0,
        })
        beat_num += 1

    # Group beats into bars at downbeat boundaries
    downbeat_idx_list = sorted(downbeat_indices)
    last_interval = (
        beats_list[-1]["time"] - beats_list[-2]["time"] if len(beats_list) >= 2 else 60.0 / bpm
    )
    bars = []
    for bar_num, db_start in enumerate(downbeat_idx_list):
        db_end = downbeat_idx_list[bar_num + 1] if bar_num + 1 < len(downbeat_idx_list) else len(beats_list)
        bar_beats = beats_list[db_start:db_end]
        if not bar_beats:
            continue
        end_time = (
            beats_list[db_end]["time"] if db_end < len(beats_list)
            else bar_beats[-1]["time"] + last_interval
        )
        bars.append({
            "index": bar_num,
            "start_time": bar_beats[0]["time"],
            "end_time": end_time,
            "beat_times": [b["time"] for b in bar_beats],
            "beat_chords": [b["chord"] for b in bar_beats],
            "beat_chord_strengths": [b["chord_strength"] for b in bar_beats],
            "chord": bar_beats[0]["chord"],
        })

    out_path = analysis_dir / "analysis.json"
    out_path.write_text(json.dumps({
        "tempo": bpm,
        "key": key,
        "bars": bars,
        "beats": beats_list,
    }, indent=2))
    print(f"Wrote {out_path} ({len(bars)} bars, {len(beats_list)} beats)")


if __name__ == "__main__":
    main()
