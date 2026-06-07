export type StageStatus = "pending" | "done" | "error";
export type SourceType = "youtube" | "local";

export interface Track {
  id: string;
  title: string;
  artist: string | null;
  sort_order: number;
  status_download: StageStatus;
  status_stems: StageStatus;
  status_analysis: StageStatus;
  error_message: string | null;
  export_path: string | null;
  duration_ms: number | null;
  source_type: SourceType;
  source_url: string | null;
  source_path: string | null;
  created_at: string;
}

export type PipelineStage = "download" | "stems" | "analysis";
export type PipelineStatus = "started" | "done" | "error";

export interface PipelineEvent {
  track_id: string;
  stage: PipelineStage;
  status: PipelineStatus;
  message?: string;
}

export type StemKey = "vocals" | "drums" | "bass" | "other";

export interface StemState {
  muted: boolean;
  soloed: boolean;
  volume: number;
}

export type StemStateMap = Record<StemKey, StemState>;

export interface AddTrackResult {
  id: string;
  duplicate: boolean;
}

export interface StemPaths {
  vocals: string;
  drums: string;
  bass: string;
  other: string;
}

export interface SetupProgress {
  downloaded_mb: number;
  total_mb?: number;
  percent?: number;
}

export interface ProgressEntry {
  stage: string;
  status: string;
  message?: string;
}

export type ProgressMap = Record<string, ProgressEntry>;
