import { listen } from "@tauri-apps/api/event";
import {
  AUTH_STATE,
  APP_STATE,
  WS_STATUS,
  WS_LOBBY_SETUP,
  WS_LOBBY_CLOSED,
  WS_LOBBY_START,
  WS_PLAYER_RESULT,
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

export function onAppState(cb: (payload: Phase) => void): UnlistenFn {
  return safeListen<Phase>(APP_STATE, cb);
}

export function onWsStatus(cb: (payload: WsStatus) => void): UnlistenFn {
  return safeListen<WsStatus>(WS_STATUS, cb);
}

export function onLobbySetup(cb: (payload: LobbySetup) => void): UnlistenFn {
  return safeListen<LobbySetup>(WS_LOBBY_SETUP, cb);
}

export function onLobbyClosed(
  cb: (payload: LobbyClosedPayload) => void,
): UnlistenFn {
  return safeListen<LobbyClosedPayload>(WS_LOBBY_CLOSED, cb);
}

export function onLobbyStart(
  cb: (payload: LobbyStartPayload) => void,
): UnlistenFn {
  return safeListen<LobbyStartPayload>(WS_LOBBY_START, cb);
}

export function onPlayerResult(
  cb: (payload: PlayerResult) => void,
): UnlistenFn {
  return safeListen<PlayerResult>(WS_PLAYER_RESULT, cb);
}
