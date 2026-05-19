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

// Matches the Rust AuthStatePayload tagged enum:
// { state: "authenticated", user: { username } } | { state: "unauthenticated" }
export type AuthStatePayload =
  | { state: "authenticated"; user: { username: string } }
  | { state: "unauthenticated" };

// User as exposed to components — id is never sent to the webview
export interface User {
  username: string;
}

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
