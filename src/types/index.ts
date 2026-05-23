export interface User {
  username: string;
}
export const AuthState = {
  Authenticated: "authenticated",
  Unauthenticated: "unauthenticated",
} as const;
export type AuthState = (typeof AuthState)[keyof typeof AuthState];
export type AuthStatePayload =
  | { state: typeof AuthState.Authenticated; user: User }
  | { state: typeof AuthState.Unauthenticated };

export const WsStatus = {
  Connected: "connected",
  Connecting: "connecting",
  Disconnected: "disconnected",
} as const;
export type WsStatus = (typeof WsStatus)[keyof typeof WsStatus];

export const PlayerStatus = {
  Preparing: "preparing",
  InProgress: "in_progress",
  Finished: "finished",
  Forfeited: "forfeited",
} as const;
export type PlayerStatus = (typeof PlayerStatus)[keyof typeof PlayerStatus];

export const LobbyStatus = {
  Waiting: "waiting",
  InProgress: "in_progress",
  Finished: "finished",
} as const;
export type LobbyStatus = (typeof LobbyStatus)[keyof typeof LobbyStatus];

export const LobbyClosedReason = {
  Left: "Left",
  Deleted: "Deleted",
  DeletedByReferee: "DeletedByReferee",
  Kicked: "Kicked",
} as const;
export type LobbyClosedReason =
  (typeof LobbyClosedReason)[keyof typeof LobbyClosedReason];

export interface LobbySetup {
  lobby_id: string;
  lobby_status: LobbyStatus;
  code: string;
  player_status: PlayerStatus;
  stream_key: string;
  whip_url: string;
  game_name: string;
  category_name: string[];
  race_start_at: number | null;
}
export interface PlayerResult {
  player_status: PlayerStatus;
  finishing_time_ms: number | null;
  finish_position: number | null;
}
export interface LobbyClosedPayload {
  lobby_id: string;
  reason: LobbyClosedReason;
}
export interface LobbyStartPayload {
  race_start_at: number;
}
