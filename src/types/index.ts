export const AppState = {
  Unauthenticated: "Unauthenticated",
  Connecting: "Connecting",
  Idle: "Idle",
  StreamSetup: "StreamSetup",
  WaitingForStart: "WaitingForStart",
  Racing: "Racing",
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
  raceStartAt: string | null;
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

export interface LobbyStateSnapshot {
  app_state: AppState;
  lobby: LobbySetup | null;
  race_start_at: string | null;
}

export interface CountdownPayload {
  race_start_at: string;
}

export interface LobbyClosedPayload {
  lobby_id: string;
  reason: string;
}

export const AuthState = {
  Authenticated: "authenticated",
  Unauthenticated: "unauthenticated",
} as const;

export type AuthState = (typeof AuthState)[keyof typeof AuthState];

export type AuthStatePayload =
  | { state: typeof AuthState.Authenticated; user: { username: string } }
  | { state: typeof AuthState.Unauthenticated };

export interface PlayerResult {
  user_id: string;
  username: string;
  finishing_time_ms: number | null;
  forfeited: boolean;
}

export interface RaceResults {
  players: PlayerResult[];
}
