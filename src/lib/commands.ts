import { invoke } from "@tauri-apps/api/core";
import type {
  User,
  LobbySetup,
  AutosplitState,
  MonitorInfo,
  WindowInfo,
  StreamSettings,
  CaptureSource,
} from "../types";

import type { Phase } from "../store/types";

interface ClientState {
  app_state: Phase;
  lobby: LobbySetup | null;
  autosplit: AutosplitState;
}

export async function openLogin(): Promise<void> {
  return invoke("open_login");
}

export async function getCurrentUser(): Promise<User | null> {
  return invoke<User | null>("get_current_user");
}

export async function logout(): Promise<void> {
  return invoke("logout");
}

export async function retryConnection(): Promise<void> {
  return invoke("retry_connection");
}

export async function publishStream(lobbyId: string): Promise<void> {
  return invoke("publish_stream", { lobbyId });
}

export async function stopStream(lobbyId: string): Promise<void> {
  return invoke("stop_stream", { lobbyId });
}

export async function listMonitors(): Promise<MonitorInfo[]> {
  return invoke<MonitorInfo[]>("list_monitors");
}

export async function listWindows(): Promise<WindowInfo[]> {
  return invoke<WindowInfo[]>("list_windows");
}

export async function captureMonitorThumb(index: number): Promise<string> {
  return invoke<string>("capture_monitor_thumb", { index });
}

export async function captureWindowThumb(hwnd: number): Promise<string> {
  return invoke<string>("capture_window_thumb", { hwnd });
}

export async function getStreamSettings(): Promise<StreamSettings> {
  return invoke<StreamSettings>("get_stream_settings");
}

export async function setStreamSettings(
  bitrateKbps: number,
  framerate: number,
  replayDir: string,
  replayAutodelete: boolean,
  replayCasual: boolean,
  replayDeleteUploaded: boolean,
): Promise<void> {
  return invoke("set_stream_settings", {
    bitrateKbps,
    framerate,
    replayDir,
    replayAutodelete,
    replayCasual,
    replayDeleteUploaded,
  });
}

export async function getCaptureSource(): Promise<CaptureSource> {
  return invoke<CaptureSource>("get_capture_source");
}

export async function setCaptureSource(source: CaptureSource): Promise<void> {
  return invoke("set_capture_source", { source });
}

export async function restartPreview(): Promise<void> {
  return invoke("restart_preview");
}

export async function openReplayDir(): Promise<void> {
  return invoke("open_replay_dir");
}

export async function pickReplayDir(): Promise<string | null> {
  return invoke<string | null>("pick_replay_dir");
}

export async function getLobbyState(): Promise<ClientState> {
  return invoke<ClientState>("get_lobby_state");
}

export async function sendPlayerFinished(
  lobbyId: string,
  finishingTimeMs: number,
): Promise<void> {
  return invoke("send_player_finished", { lobbyId, finishingTimeMs });
}

export async function sendPlayerForfeited(lobbyId: string): Promise<void> {
  return invoke("send_player_forfeited", { lobbyId });
}

export async function acknowledgeResults(): Promise<void> {
  return invoke("acknowledge_results");
}

export async function abandonUpload(): Promise<void> {
  return invoke("abandon_upload");
}

export async function retryUpload(): Promise<void> {
  return invoke("retry_upload");
}

export async function getFinishHotkey(): Promise<string> {
  return invoke<string>("get_finish_hotkey");
}

export async function setFinishHotkey(accelerator: string): Promise<void> {
  return invoke("set_finish_hotkey", { accelerator });
}

export async function registerFinishHotkey(): Promise<void> {
  return invoke("register_finish_hotkey");
}

export async function unregisterFinishHotkey(): Promise<void> {
  return invoke("unregister_finish_hotkey");
}

export interface ClockOffset {
  offset_ms: number;
  synced_at: number;
}

export async function syncClock(force: boolean): Promise<ClockOffset> {
  return invoke<ClockOffset>("sync_clock", { force });
}

export async function hideToTray(): Promise<void> {
  return invoke("hide_to_tray");
}

export async function getSplitSegments(): Promise<string[]> {
  return invoke<string[]>("get_split_segments");
}

export async function getCurrentSplitIndex(): Promise<number> {
  return invoke<number>("get_current_split_index");
}

export async function getAutosplitState(): Promise<AutosplitState> {
  return invoke<AutosplitState>("get_autosplit_state");
}
