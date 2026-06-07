import type { StemKey } from "./types";

// Sentinel ID for an optimistically-added pending track row
export const PENDING_ID = "__pending__";

// Tauri event channel names (must match Rust emitter)
export const EVENT_PIPELINE = "pipeline";
export const EVENT_SETUP_PROGRESS = "setup:progress";

// Waveform gradient ID suffix for the master (mixed) waveform
export const MASTER_KEY = "master";

// Canonical stem iteration order
export const STEM_KEYS: readonly StemKey[] = [
  "vocals",
  "drums",
  "bass",
  "other",
];

// Screen state for the top-level library ↔ playback transition
export const Screen = {
  Library: "library",
  Playback: "playback",
} as const;
export type Screen = (typeof Screen)[keyof typeof Screen];

// Sort options exposed in the library toolbar
export const SortKey = {
  Newest: "newest",
  Oldest: "oldest",
  Title: "title",
  Artist: "artist",
} as const;
export type SortKey = (typeof SortKey)[keyof typeof SortKey];

// Direction of a section-loop marker being dragged
export const LoopMarker = {
  Start: "start",
  End: "end",
} as const;
export type LoopMarker = (typeof LoopMarker)[keyof typeof LoopMarker];

// Waveform rendering — bar count and SVG viewBox geometry
export const WAVEFORM_BAR_COUNT = 120;
export const WAVEFORM_VIEW_WIDTH = 400;
export const MASTER_VIEW_HEIGHT = 60;
export const STEM_VIEW_HEIGHT = 28;
export const MASTER_BAR_HEIGHT = 54;
export const STEM_BAR_HEIGHT = 24;
export const BAR_WIDTH = 2.2;

// Waveform stop colors: master "played" indicator, unplayed segment, muted state
export const WAVEFORM_COLOR_PLAYED = "#4caf72";
export const WAVEFORM_COLOR_UNPLAYED = "#383838";
export const WAVEFORM_COLOR_MUTED = "#2e2e2e";

// Audio engine timing
export const GAIN_SMOOTHING_SEC = 0.015;
export const SKIP_SECONDS = 10;
export const DEFAULT_LOOP_SECONDS = 10;
