import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import Playback from "./Playback.svelte";
import { invoke } from "@tauri-apps/api/core";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
  convertFileSrc: vi.fn((path) => `asset://${path}`),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
}));

// Minimal AudioBuffer that satisfies extractWaveform (needs getChannelData)
function mockBuffer() {
  return {
    duration: 10,
    getChannelData: vi.fn().mockReturnValue(new Float32Array(1000)),
  };
}

// Fresh AudioContext mock per test so spies don't bleed between tests
function makeAudioCtx() {
  return {
    createGain: vi.fn().mockReturnValue({
      gain: { value: 1, setTargetAtTime: vi.fn() },
      connect: vi.fn(),
    }),
    createBufferSource: vi.fn().mockReturnValue({
      buffer: null,
      connect: vi.fn(),
      start: vi.fn(),
      stop: vi.fn(),
      disconnect: vi.fn(),
    }),
    destination: {},
    currentTime: 0,
    state: "running",
    addEventListener: vi.fn(),
    close: vi.fn().mockResolvedValue(undefined),
    decodeAudioData: vi.fn().mockResolvedValue(mockBuffer()),
    resume: vi.fn().mockResolvedValue(undefined),
  };
}

function makeTrack(id) {
  return {
    id,
    title: `Track ${id}`,
    artist: null,
    status_download: "done",
    status_stems: "done",
    status_analysis: "done",
    error_message: null,
    duration_ms: 10000,
  };
}

let audioCtx;
let cancelAnimationFrameSpy;

beforeEach(() => {
  audioCtx = makeAudioCtx();
  vi.stubGlobal("AudioContext", vi.fn().mockReturnValue(audioCtx));

  let rafId = 0;
  vi.stubGlobal(
    "requestAnimationFrame",
    vi.fn().mockImplementation(() => ++rafId),
  );
  cancelAnimationFrameSpy = vi.fn();
  vi.stubGlobal("cancelAnimationFrame", cancelAnimationFrameSpy);

  vi.stubGlobal(
    "fetch",
    vi.fn().mockResolvedValue({
      ok: true,
      arrayBuffer: vi.fn().mockResolvedValue(new ArrayBuffer(1024)),
    }),
  );

  invoke.mockResolvedValue({
    bass: "/stems/bass.wav",
    drums: "/stems/drums.wav",
    vocals: "/stems/vocals.wav",
    other: "/stems/other.wav",
  });
});

afterEach(() => {
  vi.unstubAllGlobals();
  cleanup();
});

// Wait until the play button is enabled (audio has finished loading)
async function waitForAudio(container) {
  await waitFor(() => {
    const btn = container.querySelector(".play-btn");
    if (!btn || btn.disabled) throw new Error("audio not loaded yet");
  });
}

describe("Waveform gradient rendering", () => {
  it("renders a linearGradient inside the master waveform SVG", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    const gradient = container.querySelector("svg.waveform linearGradient");
    expect(gradient).not.toBeNull();
  });

  it("master gradient id is wf-{track.id}-master", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    expect(container.querySelector("svg.waveform linearGradient").id).toBe(
      "wf-t1-master",
    );
  });

  it("all 120 master rects use gradient URL fill, not a hex color", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    const rects = container.querySelectorAll("svg.waveform rect");
    expect(rects.length).toBe(120);
    for (const rect of rects) {
      expect(rect.getAttribute("fill")).toBe("url(#wf-t1-master)");
    }
  });

  it("renders a linearGradient inside each of the 4 stem waveform SVGs", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    const svgs = container.querySelectorAll("svg.stem-waveform");
    expect(svgs.length).toBe(4);
    for (const svg of svgs) {
      expect(svg.querySelector("linearGradient")).not.toBeNull();
    }
  });

  it("stem gradient IDs match expected keys", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    const ids = Array.from(
      container.querySelectorAll("svg.stem-waveform linearGradient"),
    ).map((g) => g.id);
    for (const key of ["vocals", "drums", "bass", "other"]) {
      expect(ids).toContain(`wf-t1-${key}`);
    }
  });

  it("all 120 rects in each stem waveform use gradient URL fill", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    for (const svg of container.querySelectorAll("svg.stem-waveform")) {
      const rects = svg.querySelectorAll("rect");
      expect(rects.length).toBe(120);
      for (const rect of rects) {
        expect(rect.getAttribute("fill")).toMatch(/^url\(#wf-/);
      }
    }
  });

  it("master gradient has exactly 2 stops", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    const stops = container.querySelectorAll(
      "svg.waveform linearGradient stop",
    );
    expect(stops.length).toBe(2);
  });

  it("master gradient stops are both at 0% on initial render (playhead=0)", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    const stops = container.querySelectorAll(
      "svg.waveform linearGradient stop",
    );
    expect(stops[0].getAttribute("offset")).toBe("0%");
    expect(stops[1].getAttribute("offset")).toBe("0%");
  });

  it("updates gradient stop offsets when playhead advances", async () => {
    let capturedTick;
    vi.mocked(requestAnimationFrame).mockImplementationOnce((cb) => {
      capturedTick = cb;
      return 1;
    });

    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });
    await waitForAudio(container);

    await fireEvent.click(container.querySelector(".play-btn"));
    expect(capturedTick).toBeDefined();

    // Advance audio time to half the 10s track (duration comes from mockBuffer.duration = 10)
    audioCtx.currentTime = 5;
    capturedTick();

    await waitFor(() => {
      const stops = container.querySelectorAll(
        "svg.waveform linearGradient stop",
      );
      expect(stops[0].getAttribute("offset")).toBe("50%");
      expect(stops[1].getAttribute("offset")).toBe("50%");
    });
  });

  it("muted stem gradient uses muted color for both stops", async () => {
    const { container } = render(Playback, {
      track: makeTrack("t1"),
      active: true,
      onBack: vi.fn(),
    });

    // Mute the vocals stem (first mute button)
    await fireEvent.click(container.querySelectorAll('[title="Mute"]')[0]);

    await waitFor(() => {
      const gradient = container.querySelector(
        'linearGradient[id="wf-t1-vocals"]',
      );
      const stops = gradient.querySelectorAll("stop");
      expect(stops[0].getAttribute("stop-color")).toBe("#2e2e2e");
      expect(stops[1].getAttribute("stop-color")).toBe("#2e2e2e");
    });
  });
});

describe("Playback resource management", () => {
  it("cancels the existing RAF loop when switching to a different track", async () => {
    const { container, rerender } = render(Playback, {
      track: makeTrack("a"),
      active: true,
      onBack: vi.fn(),
    });

    await waitForAudio(container);

    // Start playback — schedTick() → requestAnimationFrame, setting rafId
    await fireEvent.click(container.querySelector(".play-btn"));
    expect(requestAnimationFrame).toHaveBeenCalled();

    // Reset spy so we only capture cancellations from the track switch
    cancelAnimationFrameSpy.mockClear();

    // Switch to a different track — loadAudio() should call cancelTick()
    await rerender({ track: makeTrack("b"), active: true, onBack: vi.fn() });

    await waitFor(() => expect(cancelAnimationFrameSpy).toHaveBeenCalled());
  });

  it("closes the AudioContext when the component unmounts", async () => {
    const { container, unmount } = render(Playback, {
      track: makeTrack("a"),
      active: true,
      onBack: vi.fn(),
    });

    // Wait for audio to load so AudioContext is created
    await waitForAudio(container);

    unmount();

    expect(audioCtx.close).toHaveBeenCalled();
  });
});
