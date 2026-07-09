import { useEffect, useRef } from "react";
import { useQueryClient } from "@tanstack/react-query";
import { useAppDispatch, useWhipRef } from "./AppContext";
import { ActionType, Phase } from "./types";
import { AuthState, PlayerStatus, type WsStatus } from "../types";
import { ensureClockFresh, resyncClock } from "../hooks/useClockOffset";
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
  onLobbySetup,
  onLobbyClosed,
  onLobbyStart,
  onPlayerResult,
  onAutosplitProbe,
  onSplitLoaded,
  onSplitFired,
} from "../lib/listeners";

export function AppEventBridge(): null {
  const dispatch = useAppDispatch();
  const whipRef = useWhipRef();
  const qc = useQueryClient();
  const lobbyIdRef = useRef<string | null>(null);

  useEffect(() => {
    const unsubs = [
      onAuthState((payload) => {
        if (payload.state === AuthState.Authenticated) {
          dispatch({ type: ActionType.AuthOk, user: payload.user });
        } else {
          whipRef.current?.stop();
          whipRef.current = null;
          dispatch({ type: ActionType.Logout });
        }
      }),

      onWsStatus((ws_status: WsStatus) => {
        dispatch({ type: ActionType.WsStatus, ws_status: ws_status });
      }),

      // Only the connection-level terminal phases; other app:state emits are driven by dedicated events.
      onAppState((phase) => {
        if (phase === Phase.ServerUnavailable) {
          whipRef.current?.stop();
          whipRef.current = null;
          dispatch({ type: ActionType.ServerUnavailable });
        } else if (phase === Phase.Banned) {
          whipRef.current?.stop();
          whipRef.current = null;
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
      }),

      onLobbyClosed((payload) => {
        lobbyIdRef.current = null;
        whipRef.current?.stop();
        whipRef.current = null;
        playSound(Sound.LobbyClosed);
        dispatch({ type: ActionType.LobbyClosed, reason: payload.reason });
      }),

      onLobbyStart((payload) => {
        // resync the clock
        void resyncClock(qc);
        dispatch({
          type: ActionType.LobbyStart,
          raceStartAt: payload.race_start_at,
        });
      }),

      onPlayerResult((result) => {
        lobbyIdRef.current = null;
        whipRef.current?.stop();
        whipRef.current = null;
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
  }, [dispatch, whipRef, qc]);

  return null;
}
