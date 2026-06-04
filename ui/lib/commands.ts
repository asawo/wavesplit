import { invoke } from "@tauri-apps/api/core";
import type { Track, AddTrackResult, StemPaths } from "./types";

export function checkDemucs(): Promise<boolean> {
  return invoke<boolean>("check_demucs");
}

export function listTracks(): Promise<Track[]> {
  return invoke<Track[]>("list_tracks");
}

export function addTrackYoutube(url: string): Promise<AddTrackResult> {
  return invoke<AddTrackResult>("add_track_youtube", { url });
}

export function addTrackLocal(path: string): Promise<AddTrackResult> {
  return invoke<AddTrackResult>("add_track_local", { path });
}

export function getStemPaths(trackId: string): Promise<StemPaths> {
  return invoke<StemPaths>("get_stem_paths", { trackId });
}

export function exportStems(
  trackId: string,
  destDir: string,
): Promise<string[]> {
  return invoke<string[]>("export_stems", { trackId, destDir });
}

export function updateTrackMeta(
  id: string,
  title: string,
  artist: string | null,
): Promise<void> {
  return invoke<void>("update_track_meta", { id, title, artist });
}

export function openFolder(path: string): Promise<void> {
  return invoke<void>("open_folder", { path });
}

export function deleteTrack(id: string): Promise<void> {
  return invoke<void>("delete_track", { id });
}

export function retryTrack(id: string): Promise<void> {
  return invoke<void>("retry_track", { id });
}

export function downloadDemucs(): Promise<void> {
  return invoke<void>("download_demucs");
}
