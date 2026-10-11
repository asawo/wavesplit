import { describe, it, expect, vi, afterEach } from "vitest";
import { render, fireEvent, cleanup } from "@testing-library/svelte";
import { flushSync } from "svelte";
import ErrorModal from "./ErrorModal.svelte";
import { errors, reportError } from "./errorReport.svelte";

vi.mock("@tauri-apps/api/app", () => ({
  getVersion: vi.fn().mockResolvedValue("0.2.6"),
}));

vi.mock("@tauri-apps/plugin-opener", () => ({
  openUrl: vi.fn().mockResolvedValue(undefined),
}));

afterEach(() => {
  cleanup();
  errors.queue.length = 0;
});

describe("ErrorModal", () => {
  it("renders nothing when there are no errors", () => {
    const { container } = render(ErrorModal);
    expect(container.querySelector(".modal")).toBeNull();
  });

  it("shows the reported title, message and context", () => {
    const { container } = render(ErrorModal);
    reportError("Export failed", new Error("disk full"), { "Track ID": "t1" });
    flushSync();
    expect(container.querySelector("h2")!.textContent).toContain(
      "Export failed",
    );
    expect(container.querySelector(".message")!.textContent).toBe("disk full");
    expect(container.querySelector(".details")!.textContent).toContain("t1");
  });

  it("collapses identical consecutive reports", () => {
    reportError("Export failed", "disk full");
    reportError("Export failed", "disk full");
    expect(errors.queue).toHaveLength(1);
  });

  it("steps through queued errors on dismiss", async () => {
    const { container } = render(ErrorModal);
    reportError("First", "a");
    reportError("Second", "b");
    flushSync();
    expect(container.querySelector(".remaining")!.textContent).toBe("+1 more");

    await fireEvent.click(container.querySelector(".export-btn")!);
    expect(container.querySelector("h2")!.textContent).toContain("Second");

    await fireEvent.click(container.querySelector(".export-btn")!);
    expect(container.querySelector(".modal")).toBeNull();
  });
});
