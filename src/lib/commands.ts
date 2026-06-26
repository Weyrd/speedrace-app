import { invoke } from "@tauri-apps/api/core";
import type { User, LobbySetup, AutosplitState } from "../types";

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

export async function sendStreamReady(lobbyId: string): Promise<void> {
  return invoke("send_stream_ready", { lobbyId });
}

export async function sendStreamStopped(lobbyId: string): Promise<void> {
  return invoke("send_stream_stopped", { lobbyId });
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
