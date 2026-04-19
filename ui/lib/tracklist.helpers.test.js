import { describe, it, expect } from "vitest";
import {
  fuzzy,
  SORT_FNS,
  stageLabel,
  nextStage,
  isReady,
  hasError,
  progressPct,
  statusLabel,
  STAGE_PROGRESS,
} from "./tracklist.helpers.js";

const track = (overrides = {}) => ({
  id: "abc",
  title: "My Track",
  artist: null,
  sort_order: 1,
  status_download: "done",
  status_stems: "done",
  status_analysis: "done",
  error_message: null,
  ...overrides,
});

describe("fuzzy", () => {
  it("matches exact string", () => {
    expect(fuzzy("bass", "bass guitar")).toBe(true);
  });

  it("matches subsequence", () => {
    expect(fuzzy("bg", "bass guitar")).toBe(true);
  });

  it("is case-insensitive", () => {
    expect(fuzzy("BASS", "bass guitar")).toBe(true);
  });

  it("returns false when query does not match", () => {
    expect(fuzzy("xyz", "bass guitar")).toBe(false);
  });

  it("empty query always matches", () => {
    expect(fuzzy("", "anything")).toBe(true);
  });
});

describe("SORT_FNS", () => {
  const a = track({ title: "Alpha", artist: "Arlo", sort_order: 1 });
  const b = track({ title: "Beta", artist: "Zara", sort_order: 2 });

  it("newest: higher sort_order first", () => {
    expect(SORT_FNS.newest(a, b)).toBeGreaterThan(0);
    expect(SORT_FNS.newest(b, a)).toBeLessThan(0);
  });

  it("oldest: lower sort_order first", () => {
    expect(SORT_FNS.oldest(a, b)).toBeLessThan(0);
  });

  it("title: alphabetical", () => {
    expect(SORT_FNS.title(a, b)).toBeLessThan(0);
  });

  it("artist: alphabetical", () => {
    expect(SORT_FNS.artist(a, b)).toBeLessThan(0);
  });
});

describe("stageLabel", () => {
  it("returns human label for known stages", () => {
    expect(stageLabel("download")).toBe("Downloading…");
    expect(stageLabel("stems")).toBe("Separating stems…");
    expect(stageLabel("analysis")).toBe("Analyzing…");
  });

  it("falls back to stage name for unknown stage", () => {
    expect(stageLabel("custom")).toBe("custom");
  });
});

describe("nextStage", () => {
  it("download → stems", () => expect(nextStage("download")).toBe("stems"));
  it("stems → analysis", () => expect(nextStage("stems")).toBe("analysis"));
  it("analysis → undefined", () =>
    expect(nextStage("analysis")).toBeUndefined());
});

describe("isReady", () => {
  it("true when analysis is done", () => {
    expect(isReady(track())).toBe(true);
  });

  it("false when analysis is pending", () => {
    expect(isReady(track({ status_analysis: "pending" }))).toBe(false);
  });

  it("false when analysis is error", () => {
    expect(isReady(track({ status_analysis: "error" }))).toBe(false);
  });
});

describe("hasError", () => {
  it("false for a fully done track", () => {
    expect(hasError(track(), {})).toBe(false);
  });

  it("true when download status is error", () => {
    expect(hasError(track({ status_download: "error" }), {})).toBe(true);
  });

  it("true when stems status is error", () => {
    expect(hasError(track({ status_stems: "error" }), {})).toBe(true);
  });

  it("true when progress reports error", () => {
    const t = track({
      status_download: "pending",
      status_stems: "pending",
      status_analysis: "pending",
    });
    const progress = { abc: { status: "error", stage: "download" } };
    expect(hasError(t, progress)).toBe(true);
  });

  it("false when progress reports started", () => {
    const t = track({
      status_download: "pending",
      status_stems: "pending",
      status_analysis: "pending",
    });
    const progress = { abc: { status: "started", stage: "download" } };
    expect(hasError(t, progress)).toBe(false);
  });
});

describe("progressPct", () => {
  it("returns 100 for a ready track", () => {
    expect(progressPct(track(), {})).toBe(100);
  });

  it("returns 0 when no progress entry exists", () => {
    const t = track({
      status_analysis: "pending",
      status_stems: "pending",
      status_download: "pending",
    });
    expect(progressPct(t, {})).toBe(0);
  });

  it("returns base pct when stage is started", () => {
    const t = track({
      status_analysis: "pending",
      status_stems: "pending",
      status_download: "pending",
    });
    const progress = { abc: { stage: "stems", status: "started" } };
    expect(progressPct(t, progress)).toBe(STAGE_PROGRESS.stems);
  });

  it("returns base + 15 when stage is done", () => {
    const t = track({
      status_analysis: "pending",
      status_stems: "pending",
      status_download: "pending",
    });
    const progress = { abc: { stage: "download", status: "done" } };
    expect(progressPct(t, progress)).toBe(STAGE_PROGRESS.download + 15);
  });
});

describe("statusLabel", () => {
  it('"Ready" for fully done track', () => {
    expect(statusLabel(track(), {})).toBe("Ready");
  });

  it('"Queued" when all stages pending and no progress', () => {
    const t = track({
      status_download: "pending",
      status_stems: "pending",
      status_analysis: "pending",
    });
    expect(statusLabel(t, {})).toBe("Queued");
  });

  it('"Processing" when partially done', () => {
    const t = track({
      status_download: "done",
      status_stems: "pending",
      status_analysis: "pending",
    });
    expect(statusLabel(t, {})).toBe("Processing");
  });

  it("shows error message from DB when stems errored", () => {
    const t = track({
      status_stems: "error",
      status_analysis: "pending",
      error_message: "OOM",
    });
    expect(statusLabel(t, {})).toBe("Error (OOM)");
  });

  it('shows "unknown" when error_message is null', () => {
    const t = track({
      status_stems: "error",
      status_analysis: "pending",
      error_message: null,
    });
    expect(statusLabel(t, {})).toBe("Error (unknown)");
  });

  it("shows download stage label when download is started", () => {
    const t = track({
      status_download: "pending",
      status_stems: "pending",
      status_analysis: "pending",
    });
    const progress = { abc: { stage: "download", status: "started" } };
    expect(statusLabel(t, progress)).toBe("Downloading…");
  });

  it("shows next stage label when download is done in progress", () => {
    const t = track({
      status_download: "pending",
      status_stems: "pending",
      status_analysis: "pending",
    });
    const progress = { abc: { stage: "download", status: "done" } };
    expect(statusLabel(t, progress)).toBe("Separating stems…");
  });

  it("shows error from progress", () => {
    const t = track({
      status_download: "pending",
      status_stems: "pending",
      status_analysis: "pending",
    });
    const progress = {
      abc: { stage: "stems", status: "error", message: "demucs OOM" },
    };
    expect(statusLabel(t, progress)).toBe("Error: demucs OOM");
  });
});
