import { WsStatus, StreamStatus, StreamEventState, type User } from "../types";
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
        state.phase !== Phase.Connecting &&
        state.phase !== Phase.ServerUnavailable
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
      if (
        state.phase === Phase.WaitingForStart ||
        state.phase === Phase.RaceInProgress
      ) {
        return { ...state, lobby: action.lobby } as AppState;
      }
      if (state.phase === Phase.StreamSetup) {
        return { ...state, lobby: action.lobby };
      }
      return {
        phase: Phase.StreamSetup,
        user: s.user,
        wsStatus: s.wsStatus,
        lobby: action.lobby,
        streamStatus: StreamStatus.Idle,
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
        streamStatus: StreamStatus.Live,
        autosplit: state.autosplit,
      };
    }

    case ActionType.StreamStopped: {
      if (state.phase === Phase.WaitingForStart) {
        return {
          phase: Phase.StreamSetup,
          user: state.user,
          wsStatus: state.wsStatus,
          lobby: state.lobby,
          streamStatus: StreamStatus.Idle,
          autosplit: state.autosplit,
        };
      }
      if (state.phase === Phase.StreamSetup) {
        return { ...state, streamStatus: StreamStatus.Idle };
      }
      return state;
    }

    case ActionType.StreamStatusChanged: {
      const s = action.status;
      // before race failure/stop bounces back to setup
      if (
        (s === StreamEventState.Error || s === StreamEventState.Stopped) &&
        state.phase === Phase.WaitingForStart
      ) {
        return {
          phase: Phase.StreamSetup,
          user: state.user,
          wsStatus: state.wsStatus,
          lobby: state.lobby,
          streamStatus: StreamStatus.Idle,
          autosplit: state.autosplit,
        };
      }
      if (
        state.phase === Phase.StreamSetup ||
        state.phase === Phase.WaitingForStart ||
        state.phase === Phase.RaceInProgress
      ) {
        const streamStatus =
          s === StreamEventState.Stopped ? StreamStatus.Idle : s;
        return { ...state, streamStatus };
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
        streamStatus: state.streamStatus,
        splitIndex: 0,
        completedSegmentTimes: [],
        currentSegmentStartMs: 0,
        autosplit: state.autosplit,
      };
    }

    case ActionType.PlayerResult: {
      // Idle is allowed so a durable finish that lands after a maintenance-screen
      // recovery (ServerUnavailable -> AuthOk -> Idle) still shows the result.
      if (state.phase !== Phase.RaceInProgress && state.phase !== Phase.Idle)
        return state;
      const s = state as Extract<AppState, { user: User; wsStatus: WsStatus }>;
      return {
        phase: Phase.Finished,
        user: s.user,
        wsStatus: s.wsStatus,
        result: action.result,
        raceType:
          state.phase === Phase.RaceInProgress
            ? state.lobby.race_type
            : undefined,
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

    case ActionType.AutosplitStatus: {
      if (
        state.phase !== Phase.StreamSetup &&
        state.phase !== Phase.WaitingForStart &&
        state.phase !== Phase.RaceInProgress
      )
        return state;
      const cur = state.autosplit;
      const next = action.status;
      if (
        cur &&
        cur.wasm === next.wasm &&
        cur.livesplit === next.livesplit &&
        cur.splits_match === next.splits_match &&
        cur.run_in_progress === next.run_in_progress
      )
        return state;
      return { ...state, autosplit: next };
    }

    case ActionType.SplitFired: {
      if (state.phase !== Phase.RaceInProgress) return state;
      const times = [...state.completedSegmentTimes];
      times[action.index] = action.segmentMs;
      return {
        ...state,
        splitIndex: action.index + 1,
        completedSegmentTimes: times,
        currentSegmentStartMs: action.newStartMs,
      };
    }

    // Connection-level terminal states from the WS backend, valid from any phase.
    case ActionType.ServerUnavailable: {
      if (state.phase === Phase.ServerUnavailable) return state;
      return { phase: Phase.ServerUnavailable };
    }

    case ActionType.Banned: {
      if (state.phase === Phase.Banned) return state;
      return { phase: Phase.Banned };
    }

    default:
      return state;
  }
}
