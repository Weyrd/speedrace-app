import {
  WsStatus,
  type User,
  type LobbySetup,
  PlayerStatus,
} from "../../types";
import { Phase, ActionType, type AppAction } from "../types";

export const MOCK_USER: User = {
  username: "Mk-username",
};

export const MOCK_LOBBY: LobbySetup = {
  lobby_id: "MOCK-4821",
  lobby_status: "waiting",
  code: "MOCK",
  player_status: PlayerStatus.Preparing,
  stream_key: "MOCK-stream-key",
  whip_url: "https://stream.momentum.weyrd.space:8889/MOCK-stream/whip",
  game_name: "MOCK-Game",
  category_name: ["MOCK-Game", "Any%"],
  max_duration_minutes: 60,
  race_start_at: null,
  expires_at: Date.now() + 30 * 60 * 1000,
  game_id: "MOCK-game-id",
  category_id: "MOCK-category-id",
  split_resource_updated_at: null,
  autosplitter_updated_at: null,
};

export const MOCK_PHASE_ACTIONS: Record<Phase, () => AppAction[]> = {
  [Phase.Unauthenticated]: () => [{ type: ActionType.Logout }],

  [Phase.Connecting]: () => [{ type: ActionType.LoginStart }],

  [Phase.Idle]: () => [{ type: ActionType.AuthOk, user: MOCK_USER }],

  [Phase.StreamSetup]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, ws_status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
  ],

  [Phase.WaitingForStart]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, ws_status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
    { type: ActionType.StreamReady, stream: new MediaStream() },
  ],

  [Phase.RaceInProgress]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, ws_status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
    { type: ActionType.StreamReady, stream: new MediaStream() },
    { type: ActionType.LobbyStart, raceStartAt: Date.now() + 3000 },
  ],

  [Phase.Finished]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, ws_status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
    { type: ActionType.StreamReady, stream: new MediaStream() },
    { type: ActionType.LobbyStart, raceStartAt: Date.now() - 10_000 },
    {
      type: ActionType.PlayerResult,
      result: {
        player_status: PlayerStatus.Finished,
        finishing_time_ms: 125_430,
        finish_position: 1,
      },
    },
  ],
};
