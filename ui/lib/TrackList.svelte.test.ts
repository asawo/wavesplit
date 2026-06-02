import { describe, it, expect, vi, beforeEach } from "vitest";
import { render, fireEvent } from "@testing-library/svelte";
import { flushSync } from "svelte";
import TrackList from "./TrackList.svelte";
import type { Track } from "./types";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn().mockResolvedValue([]),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn().mockResolvedValue(() => {}),
}));

vi.mock("@tauri-apps/plugin-dialog", () => ({
  open: vi.fn(),
  confirm: vi.fn(),
}));

const readyTrack: Track = {
  id: "track-1",
  title: "Test Track",
  artist: "Test Artist",
  sort_order: 1,
  status_download: "done",
  status_stems: "done",
  status_analysis: "done",
  error_message: null,
  export_path: null,
  duration_ms: 180000,
  source_type: "local",
  source_url: null,
  source_path: null,
  created_at: "",
};

describe("TrackList keyboard play", () => {
  let onPlay: ReturnType<typeof vi.fn>;

  beforeEach(() => {
    onPlay = vi.fn();
  });

  it("Enter in a title edit input does not trigger onPlay", async () => {
    const { container } = render(TrackList, { tracks: [readyTrack], onPlay });

    // Open edit mode by clicking the title span (flushSync forces Svelte's DOM update)
    const titleEl = container.querySelector(".title")!;
    fireEvent.click(titleEl);
    flushSync();

    // The title input should now be visible
    const input = container.querySelector(".title-input");
    expect(input).not.toBeNull();

    // Press Enter — should commit the edit, not play the track
    await fireEvent.keyDown(input!, { key: "Enter" });

    expect(onPlay).not.toHaveBeenCalled();
  });
});
