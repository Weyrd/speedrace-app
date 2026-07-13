import { useMemo } from "react";
import { useAppDispatch } from "./AppContext";
import { ActionType } from "./types";
import {
  openLogin,
  logout,
  retryConnection,
  publishStream,
  stopStream,
  sendPlayerFinished,
  sendPlayerForfeited,
  acknowledgeResults,
} from "../lib/commands";
import { tryCatch } from "../lib/tryCatch";

export interface Actions {
  login(): Promise<void>;
  logout(): Promise<void>;
  publish(lobbyId: string): Promise<void>;
  stopStream(lobbyId: string): Promise<void>;
  finish(lobbyId: string, finishingTimeMs: number): Promise<void>;
  forfeit(lobbyId: string): Promise<void>;
  newRace(): Promise<void>;
  retryConnection(): Promise<void>;
}

export function useActions(): Actions {
  const dispatch = useAppDispatch();

  return useMemo(
    (): Actions => ({
      async login() {
        dispatch({ type: ActionType.LoginStart });
        const { error } = await tryCatch(openLogin());
        if (error) {
          const err = error as { type?: string; message?: string };
          console.error("[auth] open_login error", err.message || err);
          dispatch({ type: ActionType.AuthFail });
        }
      },

      async logout() {
        dispatch({ type: ActionType.Logout });
        const { error } = await tryCatch(logout());
        if (error) console.error("[auth] logout error", error);
      },

      async publish(lobbyId: string) {
        await publishStream(lobbyId);
        dispatch({ type: ActionType.StreamReady });
      },

      async stopStream(lobbyId: string) {
        const { error } = await tryCatch(stopStream(lobbyId));
        if (error) console.error("[stream] stop_stream error", error);
        dispatch({ type: ActionType.StreamStopped });
      },

      async finish(lobbyId: string, finishingTimeMs: number) {
        const { error } = await tryCatch(
          sendPlayerFinished(lobbyId, finishingTimeMs),
        );
        if (error) console.error("[race] send_player_finished error", error);
      },

      async forfeit(lobbyId: string) {
        const { error } = await tryCatch(sendPlayerForfeited(lobbyId));
        if (error) console.error("[race] send_player_forfeited error", error);
      },

      async newRace() {
        const { error } = await tryCatch(acknowledgeResults());
        if (error) console.error("[race] acknowledge_results error", error);
        dispatch({ type: ActionType.NewRace });
      },

      async retryConnection() {
        const { error } = await tryCatch(retryConnection());
        if (error) console.error("[ws] retry_connection error", error);
      },
    }),
    [dispatch],
  );
}
