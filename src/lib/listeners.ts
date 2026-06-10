import { listen } from "@tauri-apps/api/event";
import {
  AUTH_STATE,
  APP_STATE,
  WS_STATUS,
  WS_LOBBY_SETUP,
  WS_LOBBY_CLOSED,
  WS_LOBBY_START,
  WS_PLAYER_RESULT,
  WINDOW_TRAY_HINT,
} from "./events";
import type {
  AuthStatePayload,
  LobbySetup,
  LobbyClosedPayload,
  LobbyStartPayload,
  WsStatus,
  PlayerResult,
} from "../types";
import type { Phase } from "../store/types";

type UnlistenFn = () => void;
function safeListen<T>(event: string, cb: (payload: T) => void): UnlistenFn {
  let cancelled = false;
  let realUnlisten: UnlistenFn | null = null;
  listen<T>(event, (e) => cb(e.payload)).then((fn) => {
    if (cancelled) fn();
    else realUnlisten = fn;
  });
  return () => {
    cancelled = true;
    realUnlisten?.();
  };
}

export const onAuthState = (cb: (p: AuthStatePayload) => void) =>
  safeListen<AuthStatePayload>(AUTH_STATE, cb);
export const onAppState = (cb: (p: Phase) => void) =>
  safeListen<Phase>(APP_STATE, cb);
export const onWsStatus = (cb: (p: WsStatus) => void) =>
  safeListen<WsStatus>(WS_STATUS, cb);
export const onLobbySetup = (cb: (p: LobbySetup) => void) =>
  safeListen<LobbySetup>(WS_LOBBY_SETUP, cb);
export const onLobbyClosed = (cb: (p: LobbyClosedPayload) => void) =>
  safeListen<LobbyClosedPayload>(WS_LOBBY_CLOSED, cb);
export const onLobbyStart = (cb: (p: LobbyStartPayload) => void) =>
  safeListen<LobbyStartPayload>(WS_LOBBY_START, cb);
export const onPlayerResult = (cb: (p: PlayerResult) => void) =>
  safeListen<PlayerResult>(WS_PLAYER_RESULT, cb);
export const onTrayHint = (cb: () => void) =>
  safeListen<null>(WINDOW_TRAY_HINT, () => cb());
