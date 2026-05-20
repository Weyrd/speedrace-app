import { listen } from "@tauri-apps/api/event";
import {
  AUTH_STATE,
  APP_STATE,
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
  WsStatus,
  RaceResults,
} from "../types";

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

export function onAppState(cb: (payload: AppState) => void): UnlistenFn {
  return safeListen<AppState>(APP_STATE, cb);
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

export function onRaceResults(cb: (payload: RaceResults) => void): UnlistenFn {
  return safeListen<RaceResults>(WS_RACE_RESULTS, cb);
}
