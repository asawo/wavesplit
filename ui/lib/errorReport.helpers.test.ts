import { describe, it, expect } from "vitest";
import {
  errorMessage,
  formatReport,
  issueUrl,
  ISSUES_URL,
  type ErrorReport,
} from "./errorReport.helpers";

const env = { version: "0.2.6", userAgent: "TestAgent/1.0" };
const report: ErrorReport = {
  title: "Export failed",
  message: "permission denied",
  context: { "Track ID": "abc" },
  time: "2026-10-11T00:00:00.000Z",
};

describe("errorMessage", () => {
  it("uses Error.message", () => {
    expect(errorMessage(new Error("boom"))).toBe("boom");
  });

  it("passes Tauri command strings through", () => {
    expect(errorMessage("not a directory: /x")).toBe("not a directory: /x");
  });

  it("serializes plain objects", () => {
    expect(errorMessage({ code: 1 })).toBe('{"code":1}');
  });
});

describe("formatReport", () => {
  it("includes title, message, env and context", () => {
    const text = formatReport(report, env);
    expect(text).toContain("Export failed");
    expect(text).toContain("permission denied");
    expect(text).toContain("Version: 0.2.6");
    expect(text).toContain("Platform: TestAgent/1.0");
    expect(text).toContain("Track ID: abc");
  });

  it("falls back to unknown version", () => {
    expect(formatReport(report, { ...env, version: "" })).toContain(
      "Version: unknown",
    );
  });
});

describe("issueUrl", () => {
  it("prefills title and body on the repo's new-issue page", () => {
    const url = new URL(issueUrl(report, env));
    expect(`${url.origin}${url.pathname}`).toBe(ISSUES_URL);
    expect(url.searchParams.get("title")).toBe("Export failed");
    expect(url.searchParams.get("body")).toContain("permission denied");
  });

  it("truncates very long messages", () => {
    const long = { ...report, message: "x".repeat(20000) };
    const body = new URL(issueUrl(long, env)).searchParams.get("body")!;
    expect(body.length).toBeLessThan(7000);
    expect(body).toContain("truncated");
  });
});
