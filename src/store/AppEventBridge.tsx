import { useEffect } from "react";
import { useAppDispatch, useWhipRef } from "./AppContext";
import { ActionType } from "./types";
import { AuthState, type WsStatus } from "../types";
import {
  onAuthState,
  onWsStatus,
  onLobbySetup,
  onLobbyClosed,
  onLobbyStart,
  onPlayerResult,
} from "../lib/listeners";

export function AppEventBridge(): null {
  const dispatch = useAppDispatch();
  const whipRef = useWhipRef();

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

      onLobbySetup((lobby) => {
        dispatch({ type: ActionType.LobbySetup, lobby });
      }),

      onLobbyClosed((payload) => {
        whipRef.current?.stop();
        whipRef.current = null;
        dispatch({ type: ActionType.LobbyClosed, reason: payload.reason });
      }),

      onLobbyStart((payload) => {
        dispatch({
          type: ActionType.LobbyStart,
          raceStartAt: payload.race_start_at,
        });
      }),

      onPlayerResult((result) => {
        whipRef.current?.stop();
        whipRef.current = null;
        dispatch({ type: ActionType.PlayerResult, result });
      }),
    ];

    return () => unsubs.forEach((fn) => fn());
  }, [dispatch, whipRef]);

  return null;
}
