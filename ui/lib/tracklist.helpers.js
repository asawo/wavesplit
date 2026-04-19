export function fuzzy(query, target) {
  query = query.toLowerCase();
  target = target.toLowerCase();
  let qi = 0;
  for (let i = 0; i < target.length && qi < query.length; i++) {
    if (target[i] === query[qi]) qi++;
  }
  return qi === query.length;
}

export const SORT_FNS = {
  newest: (a, b) => b.sort_order - a.sort_order,
  oldest: (a, b) => a.sort_order - b.sort_order,
  title: (a, b) => a.title.localeCompare(b.title),
  artist: (a, b) => (a.artist ?? "").localeCompare(b.artist ?? ""),
};

export function stageLabel(stage) {
  return (
    {
      download: "Downloading…",
      stems: "Separating stems…",
      analysis: "Analyzing…",
    }[stage] ?? stage
  );
}

export function nextStage(stage) {
  return { download: "stems", stems: "analysis" }[stage];
}

export function isReady(track) {
  return track.status_analysis === "done";
}

export function hasError(track, progress) {
  const p = progress?.[track.id];
  if (p?.status === "error") return true;
  return (
    track.status_download === "error" ||
    track.status_stems === "error" ||
    track.status_analysis === "error"
  );
}

export const STAGE_PROGRESS = { download: 15, stems: 50, analysis: 85 };

export function progressPct(track, progress) {
  if (isReady(track)) return 100;
  const p = progress?.[track.id];
  if (!p) return 0;
  const base = STAGE_PROGRESS[p.stage] ?? 0;
  return p.status === "done" ? base + 15 : base;
}

export function statusLabel(track, progress) {
  const p = progress?.[track.id];
  if (p) {
    if (p.status === "error") return `Error: ${p.message ?? p.stage}`;
    if (p.status === "started") return stageLabel(p.stage);
    if (p.status === "done" && p.stage !== "analysis")
      return stageLabel(nextStage(p.stage));
  }
  if (track.status_analysis === "done") return "Ready";
  if (
    track.status_stems === "error" ||
    track.status_download === "error" ||
    track.status_analysis === "error"
  ) {
    return `Error (${track.error_message ?? "unknown"})`;
  }
  if (
    track.status_analysis === "pending" &&
    track.status_stems === "pending" &&
    track.status_download === "pending"
  ) {
    return "Queued";
  }
  return "Processing";
}
