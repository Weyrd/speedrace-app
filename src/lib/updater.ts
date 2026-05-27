import { check } from "@tauri-apps/plugin-updater";
import { relaunch } from "@tauri-apps/plugin-process";

export async function checkForUpdate() {
  try {
    const update = await check();
    if (!update?.available) return;

    const yes = window.confirm(
      `Version ${update.version} disponible.\n\n${update.body ?? ""}\n\nInstaller maintenant ?`
    );
    if (!yes) return;

    await update.downloadAndInstall();
    await relaunch();
  } catch (e) {
    console.error("Update check failed:", e);
  }
}
