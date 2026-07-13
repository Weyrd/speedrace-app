/* Tuto :
Tauri émet un event, AppEventBridge écoute et dispatch une action
appReducer calcule le nouvel AppState et React re-render

AppAction :  liste de tout ce qui peut se passer dans l'app
Ex : { type: "AUTH_OK", user }

appReducer : fonctions qui prend l'état et une action -> renvoie le nouvel état
Signature : (state, action) => newState

AppState : l'état actuel de l'app, une union de phases.
ex: { phase: "Idle", user, wsStatus } ou { phase: "StreamSetup", user, wsStatus, lobby }

AppEventBridge : bridge Tauri  React,  listen events Tauri into actions.
ex : onLobbySetup((lobby) => dispatch({ type: ActionType.LobbySetup, lobby }))

Exemple quand un lobby ready :
backend envoie "ws://lobby_setup" avec les data du lobby
AppEventBridge catch via onLobbySetup() et dispatch { type: "LOBBY_SETUP", lobby }
appReducer voit qu'on est Idle et renvoie { phase: "StreamSetup", user, wsStatus, lobby }
React affiche StreamSetup
*/

import type {
  User,
  WsStatus,
  LobbySetup,
  LobbyClosedReason,
  PlayerResult,
  AutosplitState,
  StreamStatus,
  StreamEventState,
  RaceType,
} from "../types";

export const Phase = {
  Unauthenticated: "Unauthenticated",
  Connecting: "Connecting",
  Idle: "Idle",
  StreamSetup: "StreamSetup",
  WaitingForStart: "WaitingForStart",
  RaceInProgress: "RaceInProgress",
  Finished: "Finished",
  ServerUnavailable: "ServerUnavailable",
  Banned: "Banned",
} as const;
export type Phase = (typeof Phase)[keyof typeof Phase];

export type AppState =
  | { phase: typeof Phase.Unauthenticated }
  | { phase: typeof Phase.Connecting }
  | { phase: typeof Phase.Idle; user: User; wsStatus: WsStatus }
  | {
      phase: typeof Phase.StreamSetup;
      user: User;
      wsStatus: WsStatus;
      lobby: LobbySetup;
      streamStatus: StreamStatus;
      autosplit?: AutosplitState;
    }
  | {
      phase: typeof Phase.WaitingForStart;
      user: User;
      wsStatus: WsStatus;
      lobby: LobbySetup;
      streamStatus: StreamStatus;
      autosplit?: AutosplitState;
    }
  | {
      phase: typeof Phase.RaceInProgress;
      user: User;
      wsStatus: WsStatus;
      lobby: LobbySetup;
      raceStartAt: number;
      streamStatus: StreamStatus;
      splitIndex: number;
      completedSegmentTimes: number[];
      currentSegmentStartMs: number;
      autosplit?: AutosplitState;
    }
  | {
      phase: typeof Phase.Finished;
      user: User;
      wsStatus: WsStatus;
      result: PlayerResult;
      raceType?: RaceType;
    }
  | { phase: typeof Phase.ServerUnavailable }
  | { phase: typeof Phase.Banned };

export const ActionType = {
  LoginStart: "LOGIN_START",
  AuthOk: "AUTH_OK",
  AuthFail: "AUTH_FAIL",
  Logout: "LOGOUT",
  WsStatus: "WS_STATUS",
  LobbySetup: "LOBBY_SETUP",
  LobbyClosed: "LOBBY_CLOSED",
  LobbyStart: "LOBBY_START",
  PlayerResult: "PLAYER_RESULT",
  StreamReady: "STREAM_READY",
  StreamStopped: "STREAM_STOPPED",
  StreamStatusChanged: "STREAM_STATUS_CHANGED",
  NewRace: "NEW_RACE",
  AutosplitStatus: "AUTOSPLIT_STATUS",
  SplitFired: "SPLIT_FIRED",
  ServerUnavailable: "SERVER_UNAVAILABLE",
  Banned: "BANNED",
} as const;
export type ActionType = (typeof ActionType)[keyof typeof ActionType];

export type AppAction =
  | { type: typeof ActionType.LoginStart }
  | { type: typeof ActionType.AuthOk; user: User }
  | { type: typeof ActionType.AuthFail }
  | { type: typeof ActionType.Logout }
  | { type: typeof ActionType.WsStatus; ws_status: WsStatus }
  | { type: typeof ActionType.LobbySetup; lobby: LobbySetup }
  | { type: typeof ActionType.LobbyClosed; reason: LobbyClosedReason }
  | { type: typeof ActionType.LobbyStart; raceStartAt: number }
  | { type: typeof ActionType.PlayerResult; result: PlayerResult }
  | { type: typeof ActionType.StreamReady }
  | { type: typeof ActionType.StreamStopped }
  | {
      type: typeof ActionType.StreamStatusChanged;
      status: StreamEventState;
    }
  | { type: typeof ActionType.NewRace }
  | { type: typeof ActionType.AutosplitStatus; status: AutosplitState }
  | {
      type: typeof ActionType.SplitFired;
      index: number;
      segmentMs: number;
      newStartMs: number;
    }
  | { type: typeof ActionType.ServerUnavailable }
  | { type: typeof ActionType.Banned };
