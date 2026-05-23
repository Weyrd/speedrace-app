/* Tuto :
Tauri backend → émet un event
AppEventBridge l'attrape → dispatch(action)
appReducer → nouvel AppState → React re-render

AppAction — c'est juste une liste de tout ce qui peut "se passer" dans l'app :
- { type: "AUTH_OK", user: ... }

appReducer — c'est une fonction pure qui reçoit l'état actuel + une action, et retourne le nouvel état :
- (state, action) => newState

AppState — c'est l'état actuel de l'app, une union de phases :
- { phase: "Idle", user, wsStatus }
- { phase: "StreamSetup", user, wsStatus, lobby }

AppEventBridge — c'est le pont entre Tauri et React. Il écoute les events Tauri et les traduit en actions :
onLobbySetup((lobby) => {
  dispatch({ type: ActionType.LobbySetup, lobby }) // → reducer
})

Ex :
1. Backend envoie "ws://lobby_setup" avec les données du lobby
2. AppEventBridge attrape l'event via onLobbySetup()
3. Il dispatch({ type: "LOBBY_SETUP", lobby })
4. appReducer reçoit ça, voit qu'on est en Idle
   → retourne { phase: "StreamSetup", user, wsStatus, lobby }
5. React voit le nouvel état → affiche le composant StreamSetup
*/

import type {
  User,
  WsStatus,
  LobbySetup,
  LobbyClosedReason,
  PlayerResult,
} from "../types";

// Phase = the React app's current screen/state
// This mirrors Rust's AppState enum
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
      result: PlayerResult;
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
