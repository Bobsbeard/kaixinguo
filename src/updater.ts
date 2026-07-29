import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import { inTauri } from "./api";

export type UpdatePhase = "idle" | "downloading" | "ready";

/**
 * Check GitHub Releases (via the Tauri updater plugin) for a newer version.
 * If found, download and install it silently, then report "ready" so the UI
 * can offer a restart. Any failure (offline, endpoint not configured yet,
 * dev build) is swallowed — the dictionary must never be blocked by updates.
 */
export async function autoUpdate(
  onPhase: (phase: UpdatePhase) => void
): Promise<void> {
  if (!inTauri) return;
  try {
    const update = await check();
    if (!update) return;
    onPhase("downloading");
    await update.downloadAndInstall();
    onPhase("ready");
  } catch {
    onPhase("idle");
  }
}

export async function restartApp(): Promise<void> {
  await relaunch();
}
