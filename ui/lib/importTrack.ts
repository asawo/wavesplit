import { open as openDialog } from "@tauri-apps/plugin-dialog";
import { addTrackLocal } from "./commands";

export const ACCEPTED_AUDIO_EXTENSIONS = [
  "mp3",
  "wav",
  "flac",
  "m4a",
  "aac",
  "ogg",
] as const;

interface ImportCallbacks {
  onStarted: (title: string) => void;
  onAdded: (id: string | null) => Promise<void>;
  onError?: (message: string) => void;
}

/**
 * Import a local audio file by path. Shared by the file-picker and drag-drop
 * flows so duplicate / error handling stays consistent.
 *
 * Validates the extension (drag-drop bypasses the dialog's filter), starts
 * the pipeline, then surfaces duplicates and runtime errors through
 * `onError`. `onAdded` is always invoked once with the new id (or `null` on
 * failure) so the caller can settle any pending UI state.
 */
export async function importLocalPath(
  path: string,
  { onStarted, onAdded, onError }: ImportCallbacks,
): Promise<void> {
  const ext = path.split(".").pop()?.toLowerCase() ?? "";
  if (!(ACCEPTED_AUDIO_EXTENSIONS as readonly string[]).includes(ext)) {
    onError?.(`Unsupported file type: .${ext}`);
    return;
  }
  // Normalize backslashes for Windows paths (display only — `path` is passed as-is to the backend)
  onStarted(path.replace(/\\/g, "/").split("/").pop() ?? "Local file");
  try {
    const result = await addTrackLocal(path);
    if (result.duplicate) onError?.("This track is already in your library");
    await onAdded(result.id);
  } catch (e) {
    onError?.(String(e));
    await onAdded(null);
  }
}

/**
 * Open the native file picker and import the chosen file via
 * {@link importLocalPath}. Resolves with no side effects if the user
 * cancels the dialog.
 */
export async function pickAndImportLocal(
  callbacks: ImportCallbacks,
): Promise<void> {
  const selected = await openDialog({
    multiple: false,
    filters: [
      {
        name: "Audio",
        extensions: ACCEPTED_AUDIO_EXTENSIONS as unknown as string[],
      },
    ],
  });
  if (!selected) return;
  await importLocalPath(selected, callbacks);
}
