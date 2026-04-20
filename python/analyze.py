#!/usr/bin/env python3
"""
analyze.py <stems_dir> <source_wav> <analysis_dir>

Detects beat grid and chords from the source audio using Essentia.
Writes <analysis_dir>/analysis.json.
"""

import sys
import json
from pathlib import Path

try:
    import essentia
    import essentia.standard as es
    import numpy as np
except ImportError as e:
    print(f"Missing dependency: {e}\nRun: cd python && poetry install", file=sys.stderr)
    sys.exit(1)

SAMPLE_RATE = 44100
FRAME_SIZE = 4096
HOP_SIZE = 2048
CHORD_WINDOW = 1.5  # seconds per chord detection window
MIN_CHORD_STRENGTH = 0.1
TIME_SIGNATURE = 4


def detect_beats(audio):
    """Return (bpm_float, beat_times_array) using RhythmExtractor2013."""
    extractor = es.RhythmExtractor2013(method="multifeature")
    bpm, beats, _, _, _ = extractor(audio)
    return float(bpm), beats


def build_chord_track(stems_dir):
    """
    Compute per-frame HPCP from chordal stems (other + vocals), falling back to
    source.wav. Returns (chord_times, chords, strengths).

    Key choices that follow Essentia's recommended chord detection pipeline:
      - EqualLoudness pre-emphasis (standard for harmonic analysis)
      - HPCP with harmonics=8 (essential to distinguish major/minor and extensions)
      - Chordal stems only (no drums/bass) so HPCP isn't dominated by transients
    """
    stems_dir = Path(stems_dir)
    # Prefer chordal stems (guitar, keys, etc.) — free of drums and bass
    candidate = stems_dir / "other.wav"
    if not candidate.exists():
        candidate = stems_dir / "vocals.wav"

    loader = es.MonoLoader(filename=str(candidate), sampleRate=SAMPLE_RATE)
    audio = loader()
    audio = es.EqualLoudness()(audio)

    hop_duration = HOP_SIZE / SAMPLE_RATE

    windowing = es.Windowing(type="blackmanharris62")
    spectrum = es.Spectrum()
    spectral_peaks = es.SpectralPeaks(
        magnitudeThreshold=0.00001,
        maxPeaks=60,
        maxFrequency=3500,
        minFrequency=20,
        orderBy="magnitude",
    )
    # harmonics=8 is critical: without it HPCP only sees the fundamental and
    # cannot distinguish chord quality (major vs minor, extensions, etc.)
    hpcp_algo = es.HPCP(
        size=12,
        referenceFrequency=440.0,
        bandPreset=False,
        minFrequency=20.0,
        maxFrequency=3500.0,
        weightType="squaredCosine",
        nonLinear=False,
        windowSize=4.0 / 3.0,
        harmonics=8,
    )

    hpcps = []
    for frame in es.FrameGenerator(
        audio, frameSize=FRAME_SIZE, hopSize=HOP_SIZE, startFromZero=True
    ):
        windowed = windowing(frame)
        spec = spectrum(windowed)
        freqs, mags = spectral_peaks(spec)
        hpcps.append(hpcp_algo(freqs, mags))

    if not hpcps:
        return [], [], []

    chords_detection = es.ChordsDetection(
        hopSize=HOP_SIZE,
        windowSize=CHORD_WINDOW,
    )
    chords, strengths = chords_detection(np.array(hpcps))

    n = min(len(hpcps), len(chords))
    times = [i * hop_duration for i in range(n)]
    return times, list(chords[:n]), [float(s) for s in strengths[:n]]


def find_downbeat_phase(audio, beat_times, time_signature, sample_rate, bpm):
    """
    Return the beat offset (0..time_signature-1) where the first downbeat falls.
    Uses total low-frequency energy at each phase: beat 1 typically carries more
    energy due to kick drum hits on downbeats.
    """
    if len(beat_times) < time_signature * 2:
        return 0
    beat_dur = 60.0 / max(bpm, 1.0)
    half_win = max(1, int(beat_dur * sample_rate * 0.25))

    energies = []
    for bt in beat_times:
        center = int(float(bt) * sample_rate)
        lo = max(0, center - half_win)
        hi = min(len(audio), center + half_win)
        seg = audio[lo:hi]
        energies.append(float(np.mean(seg ** 2)) if len(seg) > 0 else 0.0)

    best_phase, best_score = 0, -1.0
    for phase in range(time_signature):
        score = sum(energies[phase::time_signature])
        if score > best_score:
            best_score = score
            best_phase = phase
    return best_phase


def detect_key(audio):
    """Return global key string like 'A minor'."""
    key_extractor = es.KeyExtractor()
    key, scale, _ = key_extractor(audio)
    return f"{key} {scale}"


def chord_at_time(chord_times, chords, strengths, target_time):
    """Binary-search for the chord label at target_time."""
    if not chord_times:
        return "—"
    lo, hi, idx = 0, len(chord_times) - 1, 0
    while lo <= hi:
        mid = (lo + hi) // 2
        if chord_times[mid] <= target_time:
            idx = mid
            lo = mid + 1
        else:
            hi = mid - 1
    return chords[idx] if strengths[idx] >= MIN_CHORD_STRENGTH else "—"


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
    loader = es.MonoLoader(filename=str(source_wav), sampleRate=SAMPLE_RATE)
    audio = loader()

    print("Detecting beats...", flush=True)
    bpm, beat_times = detect_beats(audio)
    print(f"  {bpm:.1f} bpm, {len(beat_times)} beats", flush=True)

    print("Building chord track...", flush=True)
    chord_times, chords, strengths = build_chord_track(stems_dir)
    print(f"  {len(chord_times)} chord frames", flush=True)

    print("Detecting key...", flush=True)
    key = detect_key(audio)
    print(f"  key: {key}", flush=True)

    # Build per-beat list with chord label at each beat
    beats_list = []
    for i, beat_time in enumerate(beat_times):
        chord = chord_at_time(chord_times, chords, strengths, float(beat_time))
        beats_list.append({
            "time": float(beat_time),
            "beat": (i % TIME_SIGNATURE) + 1,
            "chord": chord,
        })

    # Align to downbeat: RhythmExtractor2013 doesn't know which beat is beat 1.
    # Detect the phase with the most energy (kick drum typically lands on beat 1)
    # and trim the list so bars always start on the true downbeat.
    phase = find_downbeat_phase(audio, beat_times, TIME_SIGNATURE, SAMPLE_RATE, bpm)
    print(f"  downbeat phase: {phase}", flush=True)
    beats_list = beats_list[phase:]
    # Re-number beat positions after trimming
    for i, b in enumerate(beats_list):
        b["beat"] = (i % TIME_SIGNATURE) + 1

    # Group beats into bars
    beat_dur_estimate = 60.0 / bpm if bpm > 0 else 0.5
    bars = []
    for bar_idx in range(0, len(beats_list), TIME_SIGNATURE):
        bar_beats = beats_list[bar_idx : bar_idx + TIME_SIGNATURE]
        if not bar_beats:
            continue
        start_time = bar_beats[0]["time"]
        next_bar_start = bar_idx + TIME_SIGNATURE
        if next_bar_start < len(beats_list):
            end_time = beats_list[next_bar_start]["time"]
        else:
            end_time = bar_beats[-1]["time"] + beat_dur_estimate
        bars.append({
            "index": bar_idx // TIME_SIGNATURE,
            "start_time": start_time,
            "end_time": end_time,
            "beat_times": [b["time"] for b in bar_beats],
            "beat_chords": [b["chord"] for b in bar_beats],
            "chord": bar_beats[0]["chord"],
        })

    analysis = {
        "tempo": bpm,
        "key": key,
        "time_signature": TIME_SIGNATURE,
        "beats": beats_list,
        "bars": bars,
    }

    out_path = analysis_dir / "analysis.json"
    out_path.write_text(json.dumps(analysis, indent=2))
    print(f"Wrote {out_path} ({len(bars)} bars, {len(beats_list)} beats)")


if __name__ == "__main__":
    main()
