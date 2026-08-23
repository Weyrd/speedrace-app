import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAppDispatch } from "./AppContext";
import { ActionType, Phase } from "./types";
import { AuthState, PlayerStatus, StreamStatus, type WsStatus } from "../types";
import { ensureClockFresh, resyncClock } from "../hooks/useClockOffset";
import { getAutosplitState } from "../lib/commands";
import {
  captureSourceKey,
  effectiveEncoderKey,
  streamAttemptKey,
} from "../hooks/useStreamSettings";
import { playSound, primeCountdown, Sound } from "../lib/sound";

const COUNTDOWN_SOUNDS = [
  Sound.Countdown3,
  Sound.Countdown2,
  Sound.Countdown1,
  Sound.CountdownGo,
] as const;
import {
  onAuthState,
  onAppState,
  onWsStatus,
  onStreamStatus,
  onStreamEncoder,
  onStreamSource,
  onLobbySetup,
  onLobbyClosed,
  onLobbyStart,
  onPlayerResult,
  onAutosplitProbe,
  onSplitLoaded,
  onSplitFired,
  onUploadStatus,
} from "../lib/listeners";

export function AppEventBridge(): null {
  const dispatch = useAppDispatch();
  const qc = useQueryClient();
  const lobbyIdRef = useRef<string | null>(null);

  useEffect(() => {
    const unsubs = [
      onAuthState((payload) => {
        if (payload.state === AuthState.Authenticated) {
          dispatch({ type: ActionType.AuthOk, user: payload.user });
        } else {
          dispatch({ type: ActionType.Logout });
        }
      }),

      onWsStatus((ws_status: WsStatus) => {
        dispatch({ type: ActionType.WsStatus, ws_status: ws_status });
      }),

      onStreamStatus((payload) => {
        dispatch({
          type: ActionType.StreamStatusChanged,
          status: payload.state,
        });
        qc.setQueryData(
          streamAttemptKey,
          payload.state === StreamStatus.Connecting
            ? (payload.message ?? null)
            : null,
        );
      }),

      onStreamSource((source) => {
        qc.setQueryData(captureSourceKey, source);
      }),

      onStreamEncoder((payload) => {
        qc.setQueryData(effectiveEncoderKey, payload);
      }),

      onAppState((phase) => {
        if (phase === Phase.ServerUnavailable) {
          dispatch({ type: ActionType.ServerUnavailable });
        } else if (phase === Phase.Banned) {
          dispatch({ type: ActionType.Banned });
        }
      }),

      onLobbySetup((lobby) => {
        ensureClockFresh(qc);
        if (lobby.lobby_id !== lobbyIdRef.current) {
          lobbyIdRef.current = lobby.lobby_id;
          playSound(Sound.LobbyEnter);
          void primeCountdown(COUNTDOWN_SOUNDS);
        }
        dispatch({ type: ActionType.LobbySetup, lobby });
        void getAutosplitState()
          .then((status) =>
            dispatch({ type: ActionType.AutosplitStatus, status }),
          )
          .catch(() => {});
      }),

      onLobbyClosed((payload) => {
        lobbyIdRef.current = null;
        playSound(Sound.LobbyClosed);
        dispatch({ type: ActionType.LobbyClosed, reason: payload.reason });
      }),

      onLobbyStart((payload) => {
        void resyncClock(qc);
        dispatch({
          type: ActionType.LobbyStart,
          raceStartAt: payload.race_start_at,
        });
      }),

      onPlayerResult((result) => {
        lobbyIdRef.current = null;
        playSound(
          result.player_status === PlayerStatus.Forfeited
            ? Sound.RaceForfeit
            : Sound.RaceFinish,
        );
        dispatch({ type: ActionType.PlayerResult, result });
      }),

      onAutosplitProbe((p) => {
        dispatch({ type: ActionType.AutosplitStatus, status: p });
      }),

      onSplitLoaded(() => {
        void qc.invalidateQueries({ queryKey: ["split-segments"] });
      }),

      onUploadStatus((status) => {
        dispatch({ type: ActionType.UploadStatus, status });
      }),

      onSplitFired((p) => {
        dispatch({
          type: ActionType.SplitFired,
          index: p.index,
          segmentMs: p.segment_ms,
          newStartMs: p.new_start_ms,
        });
      }),
    ];

    return () => unsubs.forEach((fn) => fn());
  }, [dispatch, qc]);

  return null;
}
