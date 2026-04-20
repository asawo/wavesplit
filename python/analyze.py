#!/usr/bin/env python3
"""
analyze.py <stems_dir> <source_wav> <analysis_dir>

Detects beat grid and chords from the source audio using Essentia + madmom.
Writes <analysis_dir>/analysis.json.
"""

import sys
import json
from pathlib import Path

try:
    import essentia.standard as es
    import numpy as np
    from madmom.features.downbeats import DBNDownBeatTrackingProcessor, RNNDownBeatProcessor
except ImportError as e:
    print(f"Missing dependency: {e}\nRun: cd python && poetry install", file=sys.stderr)
    sys.exit(1)

SAMPLE_RATE = 44100
FRAME_SIZE = 4096
HOP_SIZE = 2048
TIME_SIGNATURE = 4
MIN_CHORD_STRENGTH = 0.15


def detect_beats(source_wav):
    """
    Return (bpm, beat_times) using madmom DBNDownBeatTrackingProcessor.
    beat_times starts at the first detected downbeat (beat_number == 1).
    """
    beats = DBNDownBeatTrackingProcessor(beats_per_bar=[TIME_SIGNATURE], fps=100)(
        RNNDownBeatProcessor()(str(source_wav))
    )
    first_db = int(np.argmax(beats[:, 1] == 1))
    beats = beats[first_db:]
    beat_times = beats[:, 0]
    bpm = 60.0 / float(np.median(np.diff(beat_times))) if len(beat_times) >= 2 else 120.0
    return float(bpm), beat_times


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
    bpm, beat_times = detect_beats(source_wav)
    print(f"  {bpm:.1f} bpm, {len(beat_times)} beats", flush=True)

    print("Computing HPCP frames...", flush=True)
    hpcps = compute_hpcps(stems_dir)
    print(f"  {len(hpcps)} HPCP frames", flush=True)

    print("Detecting chords...", flush=True)
    chords, strengths = detect_chords(hpcps, beat_times)
    print(f"  {len(chords)} beat chords", flush=True)

    print("Detecting key...", flush=True)
    key = detect_key(audio)
    print(f"  key: {key}", flush=True)

    beats_list = [
        {
            "time": float(bt),
            "beat": (i % TIME_SIGNATURE) + 1,
            "chord": chords[i] if i < len(chords) and strengths[i] >= MIN_CHORD_STRENGTH else "—",
            "chord_strength": strengths[i] if i < len(strengths) else 0.0,
        }
        for i, bt in enumerate(beat_times)
    ]

    last_interval = (
        beats_list[-1]["time"] - beats_list[-2]["time"] if len(beats_list) >= 2 else 60.0 / bpm
    )
    bars = []
    for bar_idx in range(0, len(beats_list), TIME_SIGNATURE):
        bar_beats = beats_list[bar_idx : bar_idx + TIME_SIGNATURE]
        if not bar_beats:
            continue
        next_bar_start = bar_idx + TIME_SIGNATURE
        end_time = (
            beats_list[next_bar_start]["time"]
            if next_bar_start < len(beats_list)
            else bar_beats[-1]["time"] + last_interval
        )
        bars.append({
            "index": bar_idx // TIME_SIGNATURE,
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
        "time_signature": TIME_SIGNATURE,
        "beats": beats_list,
        "bars": bars,
    }, indent=2))
    print(f"Wrote {out_path} ({len(bars)} bars, {len(beats_list)} beats)")


if __name__ == "__main__":
    main()
