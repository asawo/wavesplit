import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { addTrackLocal } from "./commands";

interface ImportCallbacks {
  onStarted: (title: string) => void;
  onAdded: (id: string | null) => Promise<void>;
  onError?: (message: string) => void;
}

export async function pickAndImportLocal({
  onStarted,
  onAdded,
  onError,
}: ImportCallbacks): Promise<void> {
  const selected = await openDialog({
    multiple: false,
    filters: [
      {
        name: "Audio",
        extensions: ["mp3", "wav", "flac", "m4a", "aac", "ogg"],
      },
    ],
  });
  if (!selected) return;
  // Normalize backslashes for Windows paths (display only — `selected` is passed as-is to the backend)
  onStarted(selected.replace(/\\/g, "/").split("/").pop() ?? "Local file");
  try {
    const result = await addTrackLocal(selected);
    if (result.duplicate) onError?.("This track is already in your library");
    await onAdded(result.id);
  } catch (e) {
    onError?.(String(e));
    await onAdded(null);
  }
}
