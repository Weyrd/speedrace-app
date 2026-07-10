import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";
import type { Update } from "@tauri-apps/plugin-updater";
import { tryCatch } from "./tryCatch";

export async function checkForUpdate(): Promise<Update | null> {
  const { data, error } = await tryCatch(check());
  if (error) {
    console.error("Update check failed:", error);
    return null;
  }
  return data ?? null;
}

export async function installUpdate(update: Update): Promise<void> {
  await update.downloadAndInstall();
  await relaunch();
}
