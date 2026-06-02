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
    vi.mocked(listen).mockReturnValue(new Promise(() => {}));

    const { container } = render(Setup, { onReady: vi.fn() });
    const btn = container.querySelector("button")!;

    fireEvent.click(btn);
    fireEvent.click(btn);

    await Promise.resolve();

    expect(listen).toHaveBeenCalledTimes(1);
  });
});

describe("Setup listener cleanup on retry", () => {
  it("calls the previous unlisten before registering a new listener on retry", async () => {
    const unlisten1 = vi.fn();
    const unlisten2 = vi.fn();

    vi.mocked(listen)
      .mockResolvedValueOnce(unlisten1)
      .mockResolvedValueOnce(unlisten2);

    vi.mocked(invoke)
      .mockRejectedValueOnce(new Error("network error"))
      .mockResolvedValueOnce(undefined);

    const { container } = render(Setup, { onReady: vi.fn() });

    await fireEvent.click(container.querySelector("button")!);
    await waitFor(() =>
      expect(container.querySelector(".error")).not.toBeNull(),
    );

    expect(unlisten1).not.toHaveBeenCalled();

    await fireEvent.click(container.querySelector("button")!);
    await waitFor(() => expect(listen).toHaveBeenCalledTimes(2));

    expect(unlisten1).toHaveBeenCalledTimes(1);
    expect(listen).toHaveBeenCalledTimes(2);
  });
});
