#!/usr/bin/env python3
"""
analyze.py <stems_dir> <source_wav> <analysis_dir>

Detects beat grid and chords from the source audio using Essentia.
Writes <analysis_dir>/analysis.json.
"""

import sys
import json
import time
from pathlib import Path

try:
    import essentia.standard as es
    import numpy as np
except ImportError as e:
    print(f"Missing dependency: {e}\nRun: cd python && poetry install", file=sys.stderr)
    sys.exit(1)

SAMPLE_RATE = 44100
FRAME_SIZE = 4096
HOP_SIZE = 2048
# TODO: detect time signature per-song; hardcoded 4/4 breaks waltzes and 6/8
TIME_SIGNATURE = 4
MIN_CHORD_STRENGTH = 0.15
MIN_BEAT_RMS = 0.01  # beats below this RMS in source audio are treated as silent


def _build_chord_templates():
    notes = ["C", "C#", "D", "Eb", "E", "F", "F#", "G", "Ab", "A", "Bb", "B"]
    # (suffix, intervals, weights) — root always 1.0, fifth 0.8, third 0.5, seventh 0.4
    types = [
        ("",     [0, 4, 7],      [1.0, 0.5, 0.8]),
        ("m",    [0, 3, 7],      [1.0, 0.5, 0.8]),
        ("7",    [0, 4, 7, 10],  [1.0, 0.5, 0.8, 0.4]),
        ("maj7", [0, 4, 7, 11],  [1.0, 0.5, 0.8, 0.4]),
        ("m7",   [0, 3, 7, 10],  [1.0, 0.5, 0.8, 0.4]),
        ("dim",  [0, 3, 6],      [1.0, 0.5, 0.8]),
        ("sus4", [0, 5, 7],      [1.0, 0.8, 0.8]),
    ]
    templates = {}
    for root_idx, root in enumerate(notes):
        for suffix, intervals, weights in types:
            vec = np.zeros(12)
            for interval, weight in zip(intervals, weights):
                vec[(root_idx + interval) % 12] = weight
            templates[f"{root}{suffix}"] = vec / np.linalg.norm(vec)
    return templates


CHORD_TEMPLATES = _build_chord_templates()


def compute_beat_energies(audio, beat_times, sample_rate):
    """Return RMS energy for each beat window [beat_times[i], beat_times[i+1])."""
    n = len(audio)
    energies = []
    for i, t in enumerate(beat_times):
        lo = int(float(t) * sample_rate)
        hi = int(float(beat_times[i + 1]) * sample_rate) if i + 1 < len(beat_times) else min(n, lo + sample_rate // 2)
        segment = audio[max(0, lo) : min(n, hi)]
        energies.append(float(np.sqrt(np.mean(segment ** 2))) if len(segment) > 0 else 0.0)
    return energies


def detect_beats(audio):
    """Return (bpm_float, beat_times_array) using RhythmExtractor2013."""
    bpm, beats, _, _, _ = es.RhythmExtractor2013(method="multifeature")(audio)
    return float(bpm), beats


def detect_downbeat_phase(audio, beat_times, time_signature):
    """
    Return (phase, confidence) where phase is the beat index (0..time_signature-1)
    of the first downbeat, and confidence is max_phase_sum / mean_phase_sum.

    Uses BeatsLoudness with a single 20–150 Hz band to isolate kick drum energy.
    Downbeats typically carry the most accumulated kick energy across all beats at
    the same bar phase. Confidence < 1.2 means the phases are nearly equal and the
    result is a weak guess.
    """
    if len(beat_times) < time_signature * 2:
        print(
            f"  warning: only {len(beat_times)} beats detected — "
            "downbeat detection unreliable, defaulting to phase 0",
            flush=True,
        )
        return 0, 1.0

    _, band_ratios = es.BeatsLoudness(
        sampleRate=SAMPLE_RATE,
        beats=beat_times,
        frequencyBands=[20.0, 150.0],  # single band isolating kick drum range
    )(audio)

    kick = np.array(band_ratios)[:, 0]
    phase_sums = [float(kick[p::time_signature].sum()) for p in range(time_signature)]
    best_phase = int(np.argmax(phase_sums))
    mean_sum = sum(phase_sums) / len(phase_sums)
    confidence = max(phase_sums) / mean_sum if mean_sum > 0 else 1.0

    if confidence < 1.2:
        print(
            f"  warning: low downbeat confidence ({confidence:.2f}) — "
            "no clear kick pattern, bar alignment may be off",
            flush=True,
        )

    return best_phase, float(confidence)


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
        for name in ("other", "vocals")
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
        nonLinear=False,
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
    Average HPCP frames within each beat window and match against chord templates
    via cosine similarity. Returns (chords, strengths) with len == len(beat_times).

    HOP_SIZE/SAMPLE_RATE maps beat timestamps to HPCP frame indices.
    """
    hop_dur = HOP_SIZE / SAMPLE_RATE
    n = len(hpcps)
    chords, strengths = [], []
    for i, t in enumerate(beat_times):
        lo = int(float(t) / hop_dur)
        hi = int(float(beat_times[i + 1]) / hop_dur) if i + 1 < len(beat_times) else n
        hi = min(max(lo + 1, hi), n)
        avg = np.mean(hpcps[lo:hi], axis=0) if lo < n else np.zeros(12)
        norm = np.linalg.norm(avg)
        if norm < 1e-6:
            chords.append("—"); strengths.append(0.0)
            continue
        normalized = avg / norm
        best, score = "—", 0.0
        for name, tmpl in CHORD_TEMPLATES.items():
            s = float(np.dot(normalized, tmpl))
            if s > score:
                score, best = s, name
        chords.append(best)
        strengths.append(score)
    return chords, strengths


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

    t0 = time.perf_counter()

    def elapsed():
        return f"[{time.perf_counter() - t0:.1f}s]"

    print("Loading audio...", flush=True)
    audio = es.MonoLoader(filename=str(source_wav), sampleRate=SAMPLE_RATE)()
    print(f"  done {elapsed()}", flush=True)

    print("Detecting beats...", flush=True)
    bpm, beat_times = detect_beats(audio)
    print(f"  {bpm:.1f} bpm, {len(beat_times)} beats {elapsed()}", flush=True)

    print("Detecting downbeat phase...", flush=True)
    phase, downbeat_confidence = detect_downbeat_phase(audio, beat_times, TIME_SIGNATURE)
    print(f"  phase offset: {phase}, confidence: {downbeat_confidence:.2f} {elapsed()}", flush=True)

    print("Computing HPCP frames...", flush=True)
    hpcps = compute_hpcps(stems_dir)
    print(f"  {len(hpcps)} HPCP frames {elapsed()}", flush=True)

    print("Detecting chords...", flush=True)
    # Pass the full beat_times (including pickup beats) so indices stay aligned.
    chords, strengths = detect_chords(hpcps, beat_times)
    assert len(chords) == len(beat_times), (
        f"detect_chords returned {len(chords)} chords for {len(beat_times)} beats"
    )
    print(f"  {len(chords)} beat chords {elapsed()}", flush=True)

    print("Detecting key...", flush=True)
    key = detect_key(audio)
    print(f"  key: {key} {elapsed()}", flush=True)

    beat_energies = compute_beat_energies(audio, beat_times, SAMPLE_RATE)

    # Build beats for the full timeline. Beats before the downbeat phase are
    # pickup beats (anacrusis) — kept so the UI can show them, but not grouped
    # into bars. Beat numbering uses Python's sign-handling modulo so pickup
    # beats naturally get the tail end of the bar (e.g. phase=2 → beats 3, 4).
    beats_list = [
        {
            "time": float(bt),
            "beat": ((i - phase) % TIME_SIGNATURE) + 1,
            "pickup": i < phase,
            "chord": (
                chords[i]
                if strengths[i] >= MIN_CHORD_STRENGTH and beat_energies[i] >= MIN_BEAT_RMS
                else "—"
            ),
            # chord_strength is intentionally the raw value even when chord == "—"
            # so the frontend can apply its own threshold if needed
            "chord_strength": strengths[i],
        }
        for i, bt in enumerate(beat_times)
    ]

    last_interval = (
        beats_list[-1]["time"] - beats_list[-2]["time"] if len(beats_list) >= 2 else 60.0 / bpm
    )

    # Bar grouping starts at `phase` — pickup beats are excluded from bars
    bar_num = 0
    bars = []
    for start_idx in range(phase, len(beats_list), TIME_SIGNATURE):
        bar_beats = beats_list[start_idx : start_idx + TIME_SIGNATURE]
        if not bar_beats:
            continue
        next_start = start_idx + TIME_SIGNATURE
        end_time = (
            beats_list[next_start]["time"]
            if next_start < len(beats_list)
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
        bar_num += 1

    out_path = analysis_dir / "analysis.json"
    out_path.write_text(json.dumps({
        "tempo": bpm,
        "key": key,
        "time_signature": TIME_SIGNATURE,
        "downbeat_confidence": downbeat_confidence,
        "beats": beats_list,
        "bars": bars,
    }, indent=2))
    print(f"Wrote {out_path} ({len(bars)} bars, {len(beats_list)} beats) {elapsed()}")


if __name__ == "__main__":
    main()
