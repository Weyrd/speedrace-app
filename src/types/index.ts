// Auth
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

// WS / connection
export const WsStatus = {
  Connected: "connected",
  Connecting: "connecting",
  Disconnected: "disconnected",
} as const;
export type WsStatus = (typeof WsStatus)[keyof typeof WsStatus];

// Domain enums - values must match Rust serde output exactly
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
} as const;
export type LobbyStatus = (typeof LobbyStatus)[keyof typeof LobbyStatus];

export const LobbyClosedReason = {
  Left: "Left",
  Deleted: "Deleted",
  DeletedByReferee: "DeletedByReferee",
  Kicked: "Kicked",
  DeletedByAdmin: "DeletedByAdmin",
} as const;
export type LobbyClosedReason =
  (typeof LobbyClosedReason)[keyof typeof LobbyClosedReason];

export interface AutosplitState {
  wasm: boolean;
  livesplit: boolean;
  splits_match?: boolean | null;
}

// Tauri event payloads
export interface LobbySetup {
  lobby_id: string;
  lobby_status: LobbyStatus;
  code: string;
  player_status: PlayerStatus;
  stream_key: string;
  whip_url: string;
  game_name: string;
  category_name: string[];
  max_duration_minutes: number;
  race_start_at: number | null;
  expires_at: number;
  game_id: string;
  category_id: string;
  split_resource_updated_at: string | null;
  autosplitter_updated_at: string | null;
}
export interface PlayerResult {
  player_status: PlayerStatus;
  finishing_time_ms: number | null;
  finish_position: number | null;
}
export interface SplitFiredPayload {
  index: number;
  segment_ms: number;
  new_start_ms: number;
}
export interface LobbyClosedPayload {
  lobby_id: string;
  reason: LobbyClosedReason;
}
export interface LobbyStartPayload {
  race_start_at: number;
  expires_at: number;
}
