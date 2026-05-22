import { invoke } from "@tauri-apps/api/core";
import type { User, ClientState } from "../types";

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

export async function sendPlayerFinished(lobbyId: string, finishingTimeMs: number): Promise<void> {
  return invoke("send_player_finished", { lobbyId, finishingTimeMs });
}

export async function sendPlayerForfeited(lobbyId: string): Promise<void> {
  return invoke("send_player_forfeited", { lobbyId });
}

export async function acknowledgeResults(): Promise<void> {
  return invoke("acknowledge_results");
}
