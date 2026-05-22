import { WsStatus, type User, type LobbySetup } from "../../types";
import { Phase, ActionType, type AppAction } from "../types";

export const MOCK_USER: User = {
  username: "Mk-username",
};

export const MOCK_LOBBY: LobbySetup = {
  lobby_id: "MOCK-4821",
  stream_key: "MOCK-stream-key",
  whip_url: "https://stream.momentum.weyrd.space:8889/MOCK-stream/whip",
  game_name: "MOCK-Game",
  category_name: ["MOCK-Game", "Any%"],
};

export const MOCK_PHASE_ACTIONS: Record<Phase, () => AppAction[]> = {
  [Phase.Unauthenticated]: () => [{ type: ActionType.Logout }],

  [Phase.Connecting]: () => [{ type: ActionType.LoginStart }],

  [Phase.Idle]: () => [{ type: ActionType.AuthOk, user: MOCK_USER }],

  [Phase.StreamSetup]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
  ],

  [Phase.WaitingForStart]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
    { type: ActionType.StreamReady },
  ],

  [Phase.RaceInProgress]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
    { type: ActionType.StreamReady },
    { type: ActionType.LobbyStart, raceStartAt: Date.now() + 3000 },
  ],

  [Phase.Finished]: () => [
    { type: ActionType.AuthOk, user: MOCK_USER },
    { type: ActionType.WsStatus, status: WsStatus.Connected },
    { type: ActionType.LobbySetup, lobby: MOCK_LOBBY },
    { type: ActionType.StreamReady },
    { type: ActionType.LobbyStart, raceStartAt: Date.now() - 60_000 },
    { type: ActionType.RaceResults },
  ],
};
