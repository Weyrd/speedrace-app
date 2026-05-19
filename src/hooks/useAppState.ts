import { useState, useEffect, useRef } from "react";
import {
  AppState,
  WsStatus,
  type AppStore,
  type AuthStatePayload,
} from "../types";
import {
  onAuthState,
  onWsStatus,
  onLobbySetup,
  onCountdown,
  onRaceResults,
  getLobbyState,
  getCurrentUser,
  openLogin,
  logout,
  notifyStreamStopped,
} from "../lib/tauri";
import type { WhipClient } from "../stream/whip";

const initialState: AppStore = {
  appState: AppState.Unauthenticated,
  user: null,
  wsStatus: WsStatus.Disconnected,
  lobby: null,
  raceStartAt: null,
};

type AuthenticatedPayload = Extract<
  AuthStatePayload,
  { state: "authenticated" }
>;
type UnauthenticatedPayload = Extract<
  AuthStatePayload,
  { state: "unauthenticated" }
>;

function matchAuthState<T>(
  payload: AuthStatePayload,
  handlers: {
    authenticated: (payload: AuthenticatedPayload) => T;
    unauthenticated: (payload: UnauthenticatedPayload) => T;
  },
): T {
  switch (payload.state) {
    case "authenticated":
      return handlers.authenticated(payload);
    case "unauthenticated":
      return handlers.unauthenticated(payload);
  }
}

export function useAppState() {
  const [store, setStore] = useState<AppStore>(initialState);

  const whipRef = useRef<WhipClient | null>(null);

  const patch = (partial: Partial<AppStore>) =>
    setStore((prev) => ({ ...prev, ...partial }));

  useEffect(() => {
    getLobbyState().then(({ app_state, lobby, race_start_at }) => {
      patch({
        appState: app_state,
        lobby,
        raceStartAt: race_start_at,
      });
      if (app_state !== AppState.Unauthenticated) {
        getCurrentUser().then((user) => {
          if (user) patch({ user });
        });
      }
    });
  }, []);

  useEffect(() => {
    const unsubs = [
      onAuthState((payload) =>
        matchAuthState(payload, {
          authenticated: (authed) => {
            patch({ appState: AppState.Connecting, user: authed.user });
          },
          unauthenticated: () => {
            // Clean up any live stream before resetting
            whipRef.current?.stop();
            whipRef.current = null;
            patch(initialState);
          },
        }),
      ),

      onWsStatus((payload) => {
        patch({ wsStatus: payload });
      }),

      onLobbySetup((payload) => {
        patch({ lobby: payload, appState: AppState.StreamSetup });
      }),

      onCountdown((payload) => {
        patch({
          raceStartAt: payload.race_start_at,
          appState: AppState.Racing,
        });
      }),

      // race finished stop stream and IDle
      onRaceResults(() => {
        whipRef.current?.stop();
        whipRef.current = null;
        patch({ appState: AppState.Idle, lobby: null, raceStartAt: null });
      }),
    ];

    return () => unsubs.forEach((fn) => fn());
  }, []);

  async function handleLogin() {
    patch({ appState: AppState.Connecting });
    try {
      await openLogin();
    } catch (e) {
      console.error("[auth] open_login error", e);
      patch({ appState: AppState.Unauthenticated });
    }
  }

  async function handleLogout() {
    whipRef.current?.stop();
    whipRef.current = null;
    patch(initialState);
    try {
      await logout();
    } catch (e) {
      console.error("[auth] logout error", e);
    }
  }

  // check is stream live
  function handleStreamReady(client: WhipClient) {
    whipRef.current = client;
    patch({ appState: AppState.WaitingForStart });
  }

  async function handleStopStream() {
    whipRef.current?.stop();
    whipRef.current = null;
    try {
      await notifyStreamStopped();
    } catch (e) {
      console.error("[stream] notify_stream_stopped error", e);
    }
    patch({ appState: AppState.Idle, lobby: null, raceStartAt: null });
  }

  return {
    store,
    handleLogin,
    handleLogout,
    handleStreamReady,
    handleStopStream,
    _patch: patch,
  };
}
