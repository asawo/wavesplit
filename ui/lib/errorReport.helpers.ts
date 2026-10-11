export const ISSUES_URL = "https://github.com/asawo/wavesplit/issues/new";

// GitHub rejects very long prefilled URLs; keep the body comfortably below that.
const MAX_ISSUE_BODY = 6000;

export interface ErrorReport {
  title: string;
  message: string;
  /** Extra key/value context (track id, stage, …) included in the report. */
  context?: Record<string, string>;
  time: string;
}

export interface ReportEnv {
  version: string;
  userAgent: string;
}

/** Normalize anything thrown (Error, Tauri command string, object) to text. */
export function errorMessage(e: unknown): string {
  if (e instanceof Error) return e.message;
  if (typeof e === "string") return e;
  try {
    return JSON.stringify(e);
  } catch {
    return String(e);
  }
}

/** Plain-text report suitable for the clipboard or an issue body. */
export function formatReport(report: ErrorReport, env: ReportEnv): string {
  const lines = [
    `**${report.title}**`,
    "",
    "```",
    report.message,
    "```",
    "",
    `- Version: ${env.version || "unknown"}`,
    `- Platform: ${env.userAgent}`,
    `- Time: ${report.time}`,
  ];
  for (const [k, v] of Object.entries(report.context ?? {})) {
    lines.push(`- ${k}: ${v}`);
  }
  return lines.join("\n");
}

/** Prefilled "new issue" URL for the project's GitHub repo. */
export function issueUrl(report: ErrorReport, env: ReportEnv): string {
  let body = formatReport(report, env);
  if (body.length > MAX_ISSUE_BODY) {
    body = body.slice(0, MAX_ISSUE_BODY) + "\n…(truncated, see copied details)";
  }
  body += "\n\n**What were you doing when this happened?**\n\n";
  const params = new URLSearchParams({ title: report.title, body });
  return `${ISSUES_URL}?${params.toString()}`;
}
