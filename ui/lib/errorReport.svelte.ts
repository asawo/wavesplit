import { errorMessage, type ErrorReport } from "./errorReport.helpers";

/**
 * Global queue of errors shown by `ErrorModal`. Anything that fails in the UI
 * should go through `reportError` so the user sees the full message and can
 * copy it / file an issue, instead of it being swallowed or shown as a
 * transient inline banner.
 */
export const errors: { queue: ErrorReport[] } = $state({ queue: [] });

export function reportError(
  title: string,
  error: unknown,
  context?: Record<string, string>,
): void {
  const message = errorMessage(error);
  const last = errors.queue[errors.queue.length - 1];
  // Collapse repeats (e.g. the same failure firing twice in a row)
  if (last && last.title === title && last.message === message) return;
  errors.queue.push({
    title,
    message,
    context,
    time: new Date().toISOString(),
  });
}

export function dismissError(): void {
  errors.queue.shift();
}
