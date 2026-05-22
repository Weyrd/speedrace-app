export const AppState = {
  Unauthenticated: "Unauthenticated",
  Connecting: "Connecting",
  Idle: "Idle",
  StreamSetup: "StreamSetup",
  WaitingForStart: "WaitingForStart",
  RaceInProgress: "RaceInProgress",
  Finished: "Finished",
} as const;

export type AppState = (typeof AppState)[keyof typeof AppState];

export const WsStatus = {
  Connected: "connected",
  Connecting: "connecting",
  Disconnected: "disconnected",
} as const;

export type WsStatus = (typeof WsStatus)[keyof typeof WsStatus];

export interface AppStore {
  appState: AppState;
  user: User | null;
  wsStatus: WsStatus;
  lobby: LobbySetup | null;
  raceStartAt: number | null;
}

export interface User {
  username: string;
}

export interface LobbySetup {
  lobby_id: string;
  game_name: string;
  category_name: string[];
  stream_key: string;
  whip_url: string;
}

export interface ClientState {
  app_state: AppState;
  lobby: LobbySetup | null;
}

export interface LobbyStartPayload {
  race_start_at: number;
}

export const LobbyClosedReason = {
  Left: "Left",
  Deleted: "Deleted",
  DeletedByReferee: "DeletedByReferee",
  Kicked: "Kicked",
} as const;

export type LobbyClosedReason =
  (typeof LobbyClosedReason)[keyof typeof LobbyClosedReason];
export interface LobbyClosedPayload {
  lobby_id: string;
  reason: LobbyClosedReason;
}

export const AuthState = {
  Authenticated: "authenticated",
  Unauthenticated: "unauthenticated",
} as const;

export type AuthState = (typeof AuthState)[keyof typeof AuthState];

export type AuthStatePayload =
  | { state: typeof AuthState.Authenticated; user: { username: string } }
  | { state: typeof AuthState.Unauthenticated };

export const PlayerStatus = {
  Preparing: "preparing",
  Ready: "ready",
  RaceInProgress: "race_in_progress",
  Finished: "finished",
  Forfeited: "forfeited",
} as const;

export type PlayerStatus = (typeof PlayerStatus)[keyof typeof PlayerStatus];

export interface PlayerResult {
  user_id: string;
  username: string;
  status: PlayerStatus;
  finishing_time_ms: number | null;
  finish_position: number | null;
}

export interface RaceResults {
  players: PlayerResult[];
}

export const LoginErrorType = {
  AlreadyInProgress: "AlreadyInProgress",
  System: "System",
} as const;

export type LoginErrorType =
  (typeof LoginErrorType)[keyof typeof LoginErrorType];

export type LoginError = {
  type: LoginErrorType;
  message?: string;
};
