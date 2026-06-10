import { invoke } from "@tauri-apps/api/core";
import type { User, LobbySetup } from "../types";

import type { Phase } from "../store/types";

interface ClientState {
  app_state: Phase;
  lobby: LobbySetup | null;
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

export async function sendStreamStopped(): Promise<void> {
  return invoke("send_stream_stopped");
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
