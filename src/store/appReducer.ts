import { WsStatus, type User } from "../types";
import { Phase, ActionType, type AppState, type AppAction } from "./types";

const PHASES_WITH_WS = new Set<Phase>([
  Phase.Idle,
  Phase.StreamSetup,
  Phase.WaitingForStart,
  Phase.RaceInProgress,
  Phase.Finished,
]);

const PHASES_WITH_USER = new Set<Phase>([
  Phase.Idle,
  Phase.StreamSetup,
  Phase.WaitingForStart,
  Phase.RaceInProgress,
  Phase.Finished,
]);

export const initialState: AppState = { phase: Phase.Unauthenticated };

export function appReducer(state: AppState, action: AppAction): AppState {
  switch (action.type) {
    case ActionType.LoginStart: {
      if (state.phase !== Phase.Unauthenticated) return state;
      return { phase: Phase.Connecting };
    }

    case ActionType.AuthOk: {
      if (
        state.phase !== Phase.Unauthenticated &&
        state.phase !== Phase.Connecting
      )
        return state;
      return {
        phase: Phase.Idle,
        user: action.user,
        wsStatus: WsStatus.Connecting,
      };
    }

    case ActionType.AuthFail: {
      if (state.phase !== Phase.Connecting) return state;
      return { phase: Phase.Unauthenticated };
    }

    case ActionType.Logout: {
      return { phase: Phase.Unauthenticated };
    }

    case ActionType.WsStatus: {
      if (!PHASES_WITH_WS.has(state.phase)) return state;
      return {
        ...(state as Extract<AppState, { wsStatus: WsStatus }>),
        wsStatus: action.ws_status,
      };
    }

    case ActionType.LobbySetup: {
      if (!PHASES_WITH_USER.has(state.phase)) return state;
      const s = state as Extract<AppState, { user: User; wsStatus: WsStatus }>;
      return {
        phase: Phase.StreamSetup,
        user: s.user,
        wsStatus: s.wsStatus,
        lobby: action.lobby,
      };
    }

    case ActionType.LobbyClosed: {
      if (!PHASES_WITH_USER.has(state.phase)) return state;
      const s = state as Extract<AppState, { user: User; wsStatus: WsStatus }>;
      return {
        phase: Phase.Idle,
        user: s.user,
        wsStatus: s.wsStatus,
      };
    }

    case ActionType.StreamReady: {
      if (state.phase !== Phase.StreamSetup) return state;
      return {
        phase: Phase.WaitingForStart,
        user: state.user,
        wsStatus: state.wsStatus,
        lobby: state.lobby,
        stream: action.stream,
      };
    }

    case ActionType.StreamStopped: {
      if (state.phase === Phase.WaitingForStart) {
        return {
          phase: Phase.StreamSetup,
          user: state.user,
          wsStatus: state.wsStatus,
          lobby: state.lobby,
        };
      }
      if (state.phase === Phase.RaceInProgress) {
        return {
          phase: Phase.Idle,
          user: state.user,
          wsStatus: state.wsStatus,
        };
      }
      return state;
    }

    case ActionType.LobbyStart: {
      if (state.phase !== Phase.WaitingForStart) return state;
      return {
        phase: Phase.RaceInProgress,
        user: state.user,
        wsStatus: state.wsStatus,
        lobby: state.lobby,
        raceStartAt: action.raceStartAt,
        stream: state.stream,
      };
    }

    case ActionType.PlayerResult: {
      if (state.phase !== Phase.RaceInProgress) return state;
      return {
        phase: Phase.Finished,
        user: state.user,
        wsStatus: state.wsStatus,
        result: action.result,
      };
    }

    case ActionType.NewRace: {
      if (state.phase !== Phase.Finished) return state;
      return {
        phase: Phase.Idle,
        user: state.user,
        wsStatus: state.wsStatus,
      };
    }

    default:
      return state;
  }
}
