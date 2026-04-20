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
TIME_SIGNATURE = 4
MIN_CHORD_STRENGTH = 0.4


def detect_beats(audio):
    """Return (bpm_float, beat_times_array) using RhythmExtractor2013."""
    extractor = es.RhythmExtractor2013(method="multifeature")
    bpm, beats, _, _, _ = extractor(audio)
    return float(bpm), beats


def compute_hpcps(stems_dir):
    """
    Load the chordal stem (other.wav, falling back to vocals.wav), apply
    EqualLoudness, and return (hpcps: np.ndarray shape (N,12), hop_duration: float).

    Uses HPCP with harmonics=8 — essential for distinguishing chord quality
    (major vs minor). Without harmonics the HPCP only captures the fundamental
    and cannot reliably identify chord type.
    """
    stems_dir = Path(stems_dir)
    candidate = stems_dir / "other.wav"
    if not candidate.exists():
        candidate = stems_dir / "vocals.wav"

    audio = es.MonoLoader(filename=str(candidate), sampleRate=SAMPLE_RATE)()
    audio = es.EqualLoudness()(audio)

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

    hop_duration = HOP_SIZE / SAMPLE_RATE
    return np.array(hpcps) if hpcps else np.zeros((0, 12)), hop_duration


def _build_chord_templates():
    """
    Build unit-normalised 12-bin HPCP templates for all 24 major/minor chords.
    Weights: root=1.0, third=0.5, fifth=0.8 — mirrors the relative perceptual
    salience of chord tones.
    """
    notes = ["C", "C#", "D", "D#", "E", "F", "F#", "G", "G#", "A", "A#", "B"]
    templates = {}
    for i, root in enumerate(notes):
        major = np.zeros(12)
        major[i % 12] = 1.0
        major[(i + 4) % 12] = 0.5  # major third
        major[(i + 7) % 12] = 0.8  # perfect fifth
        templates[root] = major / np.linalg.norm(major)

        minor = np.zeros(12)
        minor[i % 12] = 1.0
        minor[(i + 3) % 12] = 0.5  # minor third
        minor[(i + 7) % 12] = 0.8  # perfect fifth
        templates[f"{root}m"] = minor / np.linalg.norm(minor)
    return templates


CHORD_TEMPLATES = _build_chord_templates()


def hpcp_to_chord(hpcp_vector):
    """
    Cosine-similarity match of a 12-bin HPCP vector against the 24 major/minor
    chord templates. Returns the chord label (e.g. 'Am', 'G') or '—' when the
    best match is below MIN_CHORD_STRENGTH.
    """
    norm = np.linalg.norm(hpcp_vector)
    if norm < 1e-6:
        return "—"
    normalized = hpcp_vector / norm
    best, score = "—", MIN_CHORD_STRENGTH
    for chord, tmpl in CHORD_TEMPLATES.items():
        s = float(np.dot(normalized, tmpl))
        if s > score:
            score, best = s, chord
    return best


def find_downbeat_phase(audio, beat_times, time_signature, sample_rate, bpm):
    """
    Return the beat offset (0..time_signature-1) where the first downbeat falls.
    Uses total energy at each phase: beat 1 typically carries more energy due to
    kick drum hits on downbeats.
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

    print("Detecting beats...", flush=True)
    bpm, beat_times = detect_beats(audio)
    print(f"  {bpm:.1f} bpm, {len(beat_times)} beats", flush=True)

    print("Computing HPCP frames...", flush=True)
    hpcps, hop_dur = compute_hpcps(stems_dir)
    print(f"  {len(hpcps)} HPCP frames", flush=True)

    print("Detecting key...", flush=True)
    key = detect_key(audio)
    print(f"  key: {key}", flush=True)

    # Build per-beat chord list by averaging all HPCP frames within each
    # beat's time window [beat_time, next_beat_time). This gives one clean
    # HPCP per beat with no bleed from neighbouring beats, then matches it
    # against chord templates via cosine similarity.
    beat_dur_estimate = 60.0 / bpm if bpm > 0 else 0.5
    beats_list = []
    for i, beat_time in enumerate(beat_times):
        next_time = float(beat_times[i + 1]) if i + 1 < len(beat_times) else float(beat_time) + beat_dur_estimate
        lo = int(float(beat_time) / hop_dur)
        hi = max(lo + 1, int(next_time / hop_dur))
        hi = min(hi, len(hpcps))
        avg_hpcp = np.mean(hpcps[lo:hi], axis=0) if lo < len(hpcps) else np.zeros(12)
        chord = hpcp_to_chord(avg_hpcp)
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
    for i, b in enumerate(beats_list):
        b["beat"] = (i % TIME_SIGNATURE) + 1

    # Group beats into bars
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
