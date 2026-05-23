import type {
  User,
  WsStatus,
  LobbySetup,
  LobbyClosedReason,
  PlayerStatus,
} from "../types";

export const Phase = {
  Unauthenticated: "Unauthenticated",
  Connecting: "Connecting",
  Idle: "Idle",
  StreamSetup: "StreamSetup",
  WaitingForStart: "WaitingForStart",
  RaceInProgress: "RaceInProgress",
  Finished: "Finished",
} as const;

export type Phase = (typeof Phase)[keyof typeof Phase];

export interface PlayerResult {
  player_status: PlayerStatus;
  finishing_time_ms: number | null;
  finish_position: number | null;
}

export interface RaceResultEntry {
  user_id: string;
  username: string;
  player_status: PlayerStatus;
  finishing_time_ms: number | null;
  finish_position: number | null;
}

export type AppState =
  | { phase: typeof Phase.Unauthenticated }
  | { phase: typeof Phase.Connecting }
  | { phase: typeof Phase.Idle; user: User; wsStatus: WsStatus }
  | {
      phase: typeof Phase.StreamSetup;
      user: User;
      wsStatus: WsStatus;
      lobby: LobbySetup;
    }
  | {
      phase: typeof Phase.WaitingForStart;
      user: User;
      wsStatus: WsStatus;
      lobby: LobbySetup;
      stream: MediaStream;
    }
  | {
      phase: typeof Phase.RaceInProgress;
      user: User;
      wsStatus: WsStatus;
      lobby: LobbySetup;
      raceStartAt: number;
      stream: MediaStream;
    }
  | {
      phase: typeof Phase.Finished;
      user: User;
      wsStatus: WsStatus;
      results: PlayerResult;
    };

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
  NewRace: "NEW_RACE",
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
  | { type: typeof ActionType.StreamReady; stream: MediaStream }
  | { type: typeof ActionType.StreamStopped }
  | { type: typeof ActionType.NewRace };
