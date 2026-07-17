import { listen } from "@tauri-apps/api/event";
import {
  AUTH_STATE,
  APP_STATE,
  WS_STATUS,
  STREAM_STATUS,
  STREAM_PREVIEW,
  WS_LOBBY_SETUP,
  WS_LOBBY_CLOSED,
  WS_LOBBY_START,
  WS_PLAYER_RESULT,
  WINDOW_TRAY_HINT,
  SPLIT_LOADED,
  SPLIT_FIRED,
  AUTOSPLIT_PROBE,
  UPLOAD_STATUS,
} from "./events";
import type {
  AuthStatePayload,
  LobbySetup,
  LobbyClosedPayload,
  LobbyStartPayload,
  WsStatus,
  PlayerResult,
  AutosplitState,
  SplitFiredPayload,
  StreamStatusPayload,
  StreamPreviewPayload,
  UploadStatusPayload,
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
export const onStreamStatus = (cb: (p: StreamStatusPayload) => void) =>
  safeListen<StreamStatusPayload>(STREAM_STATUS, cb);
export const onStreamPreview = (cb: (p: StreamPreviewPayload) => void) =>
  safeListen<StreamPreviewPayload>(STREAM_PREVIEW, cb);
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
export const onSplitLoaded = (cb: () => void) =>
  safeListen<null>(SPLIT_LOADED, () => cb());
export const onSplitFired = (cb: (p: SplitFiredPayload) => void) =>
  safeListen<SplitFiredPayload>(SPLIT_FIRED, cb);
export const onAutosplitProbe = (cb: (p: AutosplitState) => void) =>
  safeListen<AutosplitState>(AUTOSPLIT_PROBE, cb);
export const onUploadStatus = (cb: (p: UploadStatusPayload) => void) =>
  safeListen<UploadStatusPayload>(UPLOAD_STATUS, cb);
