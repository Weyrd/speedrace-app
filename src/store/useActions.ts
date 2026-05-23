import { useMemo } from "react";
import { useAppDispatch, useWhipRef } from "./AppContext";
import { ActionType } from "./types";
import {
  openLogin,
  logout,
  sendStreamReady,
  sendStreamStopped,
  sendPlayerFinished,
  sendPlayerForfeited,
  acknowledgeResults,
} from "../lib/commands";
import type { WhipClient } from "../stream/whip";

export interface Actions {
  login(): Promise<void>;
  logout(): Promise<void>;
  streamReady(
    client: WhipClient,
    stream: MediaStream,
    lobbyId: string,
  ): Promise<void>;
  stopStream(): Promise<void>;
  finish(lobbyId: string, finishingTimeMs: number): Promise<void>;
  forfeit(lobbyId: string): Promise<void>;
  newRace(): Promise<void>;
}

export function useActions(): Actions {
  const dispatch = useAppDispatch();
  const whipRef = useWhipRef();

  return useMemo(
    (): Actions => ({
      async login() {
        dispatch({ type: ActionType.LoginStart });
        try {
          await openLogin();
        } catch (e) {
          const err = e as { type?: string; message?: string };
          console.error("[auth] open_login error", err.message || err);
          dispatch({ type: ActionType.AuthFail });
        }
      },

      async logout() {
        whipRef.current?.stop();
        whipRef.current = null;
        dispatch({ type: ActionType.Logout });
        try {
          await logout();
        } catch (e) {
          console.error("[auth] logout error", e);
        }
      },

      async streamReady(
        client: WhipClient,
        stream: MediaStream,
        lobbyId: string,
      ) {
        whipRef.current = client;
        try {
          await sendStreamReady(lobbyId);
        } catch (e) {
          console.error("[stream] send_stream_ready error", e);
        }
        dispatch({ type: ActionType.StreamReady, stream });
      },

      async stopStream() {
        whipRef.current?.stop();
        whipRef.current = null;
        try {
          await sendStreamStopped();
        } catch (e) {
          console.error("[stream] send_stream_stopped error", e);
        }
        dispatch({ type: ActionType.StreamStopped });
      },

      async finish(lobbyId: string, finishingTimeMs: number) {
        try {
          await sendPlayerFinished(lobbyId, finishingTimeMs);
        } catch (e) {
          console.error("[race] send_player_finished error", e);
        }
      },

      async forfeit(lobbyId: string) {
        whipRef.current?.stop();
        whipRef.current = null;
        try {
          await sendPlayerForfeited(lobbyId);
        } catch (e) {
          console.error("[race] send_player_forfeited error", e);
        }
      },

      async newRace() {
        try {
          await acknowledgeResults();
        } catch (e) {
          console.error("[race] acknowledge_results error", e);
        }
        dispatch({ type: ActionType.NewRace });
      },
    }),
    [dispatch, whipRef],
  );
}
