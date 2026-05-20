import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AppState,
  AuthState,
  WsStatus,
  type AppStore,
  type LobbySetup,
  type User,
} from "../types";
import {
  onAuthState,
  onAppState,
  onWsStatus,
  onLobbySetup,
  onLobbyClosed,
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

// ── Query key ────────────────────────────────────────────────────────────────

const APP_STATE_KEY = ["app:state"] as const;

// ── Server-owned state (everything except wsStatus which is client-only) ─────

type ServerStore = {
  appState: AppState;
  user: User | null;
  lobby: LobbySetup | null;
  raceStartAt: string | null;
};

const serverInitial: ServerStore = {
  appState: AppState.Unauthenticated,
  user: null,
  lobby: null,
  raceStartAt: null,
};

async function fetchAppState(): Promise<ServerStore> {
  const { app_state, lobby, race_start_at } = await getLobbyState();
  const user =
    app_state !== AppState.Unauthenticated ? await getCurrentUser() : null;
  return { appState: app_state, user, lobby, raceStartAt: race_start_at };
}

// ── Hook ─────────────────────────────────────────────────────────────────────

export function useAppState() {
  const queryClient = useQueryClient();
  const whipRef = useRef<WhipClient | null>(null);
  // Guard against calling openLogin() when a flow is already pending in Tauri.
  const loginPendingRef = useRef(false);
  // wsStatus is purely client-driven (no server fetch), so it lives in useState.
  const [wsStatus, setWsStatus] = useState<WsStatus>(WsStatus.Disconnected);

  // ── Server state via TanStack Query ──────────────────────────────────────
  // staleTime: Infinity — we never want automatic background refetches;
  // all updates come from events (setQueryData) or explicit invalidation.
  // refetchInterval polls every second while waiting for WS to come up.
  const { data: serverStore = serverInitial } = useQuery<ServerStore>({
    queryKey: APP_STATE_KEY,
    queryFn: fetchAppState,
    staleTime: Infinity,
    refetchInterval: (query) =>
      query.state.data?.appState === AppState.Connecting ? 1_000 : false,
  });

  // Helper: patch the query cache (replaces setState for server-owned fields).
  function patchServer(partial: Partial<ServerStore>) {
    queryClient.setQueryData<ServerStore>(
      APP_STATE_KEY,
      (prev = serverInitial) => ({
        ...prev,
        ...partial,
      }),
    );
  }

  // ── External system: Tauri event subscriptions ───────────────────────────
  // This is the ONLY useEffect — subscribing to an external push-based source
  // is the canonical valid use case for useEffect.
  useEffect(() => {
    const unsubs = [
      onAuthState((payload) => {
        if (payload.state === AuthState.Authenticated) {
          loginPendingRef.current = false;
          patchServer({ appState: AppState.Connecting, user: payload.user });
          // Fetch fresh server state now that we have a session.
          queryClient.invalidateQueries({ queryKey: APP_STATE_KEY });
        } else {
          whipRef.current?.stop();
          whipRef.current = null;
          loginPendingRef.current = false;
          setWsStatus(WsStatus.Disconnected);
          queryClient.setQueryData(APP_STATE_KEY, serverInitial);
        }
      }),

      onAppState((appState) => patchServer({ appState })),

      onWsStatus((status) => {
        setWsStatus(status);
        if (status === WsStatus.Connected) {
          // Sync app state from server after WS reconnect.
          queryClient.invalidateQueries({ queryKey: APP_STATE_KEY });
        }
      }),

      onLobbySetup((lobby) =>
        patchServer({ lobby, appState: AppState.StreamSetup }),
      ),

      onLobbyClosed(() => {
        whipRef.current?.stop();
        whipRef.current = null;
        patchServer({ appState: AppState.Idle, lobby: null, raceStartAt: null });
      }),

      onCountdown((payload) =>
        patchServer({
          raceStartAt: payload.race_start_at,
          appState: AppState.Racing,
        }),
      ),

      onRaceResults(() => {
        whipRef.current?.stop();
        whipRef.current = null;
        patchServer({
          appState: AppState.Idle,
          lobby: null,
          raceStartAt: null,
        });
      }),
    ];

    return () => unsubs.forEach((fn) => fn());
  }, []); // stable: queryClient and refs never change

  // ── Actions ───────────────────────────────────────────────────────────────

  async function handleLogin() {
    // Prevent calling openLogin() when a Tauri OAuth flow is already in flight.
    // Restore Connecting state so the Disconnect button stays visible.
    if (loginPendingRef.current) {
      patchServer({ appState: AppState.Connecting });
      return;
    }
    loginPendingRef.current = true;
    patchServer({ appState: AppState.Connecting });
    try {
      await openLogin();
    } catch (e) {
      const msg = e instanceof Error ? e.message : String(e);
      // "Login already in progress" means Tauri still has an active OAuth flow.
      // Stay in Connecting so the user can click Disconnect to cancel it.
      if (msg.toLowerCase().includes("already in progress")) return;
      console.error("[auth] open_login error", e);
      loginPendingRef.current = false;
      patchServer({ appState: AppState.Unauthenticated });
    }
  }

  async function handleLogout() {
    whipRef.current?.stop();
    whipRef.current = null;
    loginPendingRef.current = false;
    setWsStatus(WsStatus.Disconnected);
    queryClient.setQueryData(APP_STATE_KEY, serverInitial);
    try {
      await logout();
    } catch (e) {
      console.error("[auth] logout error", e);
    }
  }

  function handleStreamReady(client: WhipClient) {
    whipRef.current = client;
    patchServer({ appState: AppState.WaitingForStart });
  }

  async function handleStopStream() {
    whipRef.current?.stop();
    whipRef.current = null;
    try {
      await sendStreamStopped();
    } catch (e) {
      console.error("[stream] send_stream_stopped error", e);
    }
    // If we were just waiting (lobby still alive), go back to StreamSetup.
    // If racing/finished, the lobby is done — reset to Idle.
    if (serverStore.appState === AppState.WaitingForStart) {
      patchServer({ appState: AppState.StreamSetup, raceStartAt: null });
    } else {
      patchServer({ appState: AppState.Idle, lobby: null, raceStartAt: null });
    }
  }

  // ── Derived store (merge server state + client wsStatus) ─────────────────

  const store: AppStore = { ...serverStore, wsStatus };

  const isConnected = wsStatus === WsStatus.Connected;

  return {
    store,
    isConnected,
    handleLogin,
    handleLogout,
    handleStreamReady,
    handleStopStream,
    _patch: patchServer,
  };
}
