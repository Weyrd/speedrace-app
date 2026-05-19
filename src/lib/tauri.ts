import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  AUTH_STATE,
  WS_STATUS,
  WS_LOBBY_SETUP,
  WS_COUNTDOWN,
  WS_RACE_RESULTS,
} from "./events";
import type {
  AppState,
  AuthStatePayload,
  LobbySetup,
  CountdownPayload,
  User,
  WsStatus,
  LobbyStateSnapshot,
} from "../types";

type UnlistenFn = () => void;

export async function getAppState(): Promise<AppState> {
  return invoke<AppState>("get_app_state");
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

export async function notifyStreamReady(lobbyId: string): Promise<void> {
  return invoke("notify_stream_ready", { lobbyId });
}

export async function notifyStreamStopped(): Promise<void> {
  return invoke("notify_stream_stopped");
}

function safeListen<T>(event: string, cb: (payload: T) => void): UnlistenFn {
  let cancelled = false;
  let realUnlisten: UnlistenFn | null = null;

  listen<T>(event, (e) => cb(e.payload)).then((fn) => {
    if (cancelled) {
      fn();
    } else {
      realUnlisten = fn;
    }
  });

  return () => {
    cancelled = true;
    realUnlisten?.();
  };
}

export function onAuthState(
  cb: (payload: AuthStatePayload) => void,
): UnlistenFn {
  return safeListen<AuthStatePayload>(AUTH_STATE, cb);
}

export function onWsStatus(cb: (payload: WsStatus) => void): UnlistenFn {
  return safeListen<WsStatus>(WS_STATUS, cb);
}

export function onLobbySetup(cb: (payload: LobbySetup) => void): UnlistenFn {
  return safeListen<LobbySetup>(WS_LOBBY_SETUP, cb);
}

export function onCountdown(
  cb: (payload: CountdownPayload) => void,
): UnlistenFn {
  return safeListen<CountdownPayload>(WS_COUNTDOWN, cb);
}

export function onRaceResults(cb: (payload: unknown) => void): UnlistenFn {
  return safeListen<unknown>(WS_RACE_RESULTS, cb);
}

export async function getLobbyState(): Promise<LobbyStateSnapshot> {
  return invoke<LobbyStateSnapshot>("get_lobby_state");
}
