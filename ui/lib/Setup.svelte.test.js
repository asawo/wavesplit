import { describe, it, expect, vi, beforeEach, afterEach } from "vitest";
import { render, fireEvent, waitFor, cleanup } from "@testing-library/svelte";
import Setup from "./Setup.svelte";
import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";

vi.mock("@tauri-apps/api/core", () => ({
  invoke: vi.fn(),
}));

vi.mock("@tauri-apps/api/event", () => ({
  listen: vi.fn(),
}));

beforeEach(() => {
  vi.resetAllMocks();
});

afterEach(() => {
  cleanup();
});

describe("Setup concurrency guard", () => {
  it("registers only one event listener when startDownload is triggered twice in quick succession", async () => {
    // listen() never resolves, so downloading stays true for the duration of the test
    listen.mockReturnValue(new Promise(() => {}));

    const { container } = render(Setup, { onReady: vi.fn() });
    const btn = container.querySelector("button");

    // Fire two clicks without awaiting — the handler for the first click runs
    // synchronously up to `await listen()`, setting downloading=true before the
    // second click executes. The guard `if (downloading) return` must catch it.
    fireEvent.click(btn);
    fireEvent.click(btn);

    // Flush microtasks so any erroneous second listen() call would have run
    await Promise.resolve();

    expect(listen).toHaveBeenCalledTimes(1);
  });
});

describe("Setup listener cleanup on retry", () => {
  it("calls the previous unlisten before registering a new listener on retry", async () => {
    const unlisten1 = vi.fn();
    const unlisten2 = vi.fn();

    listen.mockResolvedValueOnce(unlisten1).mockResolvedValueOnce(unlisten2);

    // First attempt fails so the user can retry
    invoke.mockRejectedValueOnce(new Error("network error"));
    invoke.mockResolvedValueOnce(undefined);

    const { container } = render(Setup, { onReady: vi.fn() });

    // First download attempt — invoke rejects → error shown, downloading reset to false
    await fireEvent.click(container.querySelector("button"));
    await waitFor(() =>
      expect(container.querySelector(".error")).not.toBeNull(),
    );

    // At this point unlisten1 has been registered but not yet called
    expect(unlisten1).not.toHaveBeenCalled();

    // Retry — startDownload() should call unlisten1() before registering the new listener
    await fireEvent.click(container.querySelector("button"));
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));

    expect(unlisten1).toHaveBeenCalledTimes(1);
    expect(listen).toHaveBeenCalledTimes(2);
  });
});
