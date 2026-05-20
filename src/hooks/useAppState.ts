import { useState, useEffect, useRef } from "react";
import { AppState, AuthState, WsStatus, type AppStore } from "../types";
import {
  onAuthState,
  onAppState,
  onWsStatus,
  onLobbySetup,
  onCountdown,
  onRaceResults,
} from "../lib/listeners";
import {
  getLobbyState,
  getCurrentUser,
  openLogin,
  logout,
  sendStreamStopped,
} from "../lib/commands";
import type { WhipClient } from "../stream/whip";

const initialState: AppStore = {
  appState: AppState.Unauthenticated,
  user: null,
  wsStatus: WsStatus.Disconnected,
  lobby: null,
  raceStartAt: null,
};

type PatchFn = (partial: Partial<AppStore>) => void;


function useAppStore() {
  const [store, setStore] = useState<AppStore>(initialState);
  const patch: PatchFn = (partial) =>
    setStore((prev) => ({ ...prev, ...partial }));
  return { store, patch };
}

function useAppEvents(patch: PatchFn, whipRef: React.MutableRefObject<WhipClient | null>) {
  useEffect(() => {
    getLobbyState()
      .then(({ app_state, lobby, race_start_at }) => {
        patch({ appState: app_state, lobby, raceStartAt: race_start_at });
        if (app_state !== AppState.Unauthenticated) {
          getCurrentUser().then((user) => {
            if (user) patch({ user });
          });
        }
      })
      .catch((e) => console.error("[state] getLobbyState error:", e));
  }, []);

  useEffect(() => {
    const unsubs = [
      onAuthState((payload) => {
        if (payload.state === AuthState.Authenticated) {
          patch({ appState: AppState.Connecting, user: payload.user });
        } else {
          whipRef.current?.stop();
          whipRef.current = null;
          patch(initialState);
        }
      }),

      onAppState((appState) => patch({ appState })),

      onWsStatus((wsStatus) => patch({ wsStatus })),

      onLobbySetup((lobby) => patch({ lobby, appState: AppState.StreamSetup })),

      onCountdown((payload) =>
        patch({ raceStartAt: payload.race_start_at, appState: AppState.Racing }),
      ),

      onRaceResults(() => {
        whipRef.current?.stop();
        whipRef.current = null;
        patch({ appState: AppState.Idle, lobby: null, raceStartAt: null });
      }),
    ];

    return () => unsubs.forEach((fn) => fn());
  }, []);
}


function useAppActions(patch: PatchFn, whipRef: React.MutableRefObject<WhipClient | null>) {
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

  function handleStreamReady(client: WhipClient) {
    whipRef.current = client;
    patch({ appState: AppState.WaitingForStart });
  }

  async function handleStopStream() {
    whipRef.current?.stop();
    whipRef.current = null;
    try {
      await sendStreamStopped();
    } catch (e) {
      console.error("[stream] send_stream_stopped error", e);
    }
    patch({ appState: AppState.Idle, lobby: null, raceStartAt: null });
  }

  return { handleLogin, handleLogout, handleStreamReady, handleStopStream };
}


export function useAppState() {
  const { store, patch } = useAppStore();
  const whipRef = useRef<WhipClient | null>(null);

  useAppEvents(patch, whipRef);
  const actions = useAppActions(patch, whipRef);

  return {
    store,
    ...actions,
    _patch: patch,
  };
}

