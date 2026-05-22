import { useEffect, useRef, useState } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import {
  AppState,
  AuthState,
  WsStatus,
  type AppStore,
  type LobbySetup,
  type User,
  LoginErrorType,
  type LoginError,
} from "../types";
import {
  onAuthState,
  onAppState,
  onWsStatus,
  onLobbySetup,
  onLobbyClosed,
  onLobbyStart,
  onRaceResults,
} from "../lib/listeners";
import {
  getLobbyState,
  getCurrentUser,
  openLogin,
  logout,
  sendStreamStopped,
  sendPlayerFinished,
  sendPlayerForfeited,
  acknowledgeResults,
} from "../lib/commands";
import { APP_STATE } from "../lib/events";
import type { WhipClient } from "../stream/whip";

type ServerStore = {
  appState: AppState;
  user: User | null;
  lobby: LobbySetup | null;
  raceStartAt: number | null;
};

const serverInitial: ServerStore = {
  appState: AppState.Unauthenticated,
  user: null,
  lobby: null,
  raceStartAt: null,
};

export function useAppState() {
  const queryClient = useQueryClient();
  async function fetchAppState(): Promise<ServerStore> {
    const { app_state, lobby } = await getLobbyState();
    const user =
      app_state !== AppState.Unauthenticated ? await getCurrentUser() : null;
    const prev = queryClient.getQueryData<ServerStore>([APP_STATE]);
    return {
      appState: app_state,
      user,
      lobby,
      raceStartAt: prev?.raceStartAt ?? null,
    };
  }

  const whipRef = useRef<WhipClient | null>(null);
  const loginPendingRef = useRef(false);
  const [wsStatus, setWsStatus] = useState<WsStatus>(WsStatus.Disconnected);

  const { data: serverStore = serverInitial } = useQuery<ServerStore>({
    queryKey: [APP_STATE],
    queryFn: fetchAppState,
    staleTime: Infinity,
    refetchInterval: (query) =>
      query.state.data?.appState === AppState.Connecting ? 1_000 : false,
  });

  function patchServer(partial: Partial<ServerStore>) {
    queryClient.setQueryData<ServerStore>(
      [APP_STATE],
      (prev = serverInitial) => ({
        ...prev,
        ...partial,
      }),
    );
  }

  useEffect(() => {
    const unsubs = [
      onAuthState((payload) => {
        if (payload.state === AuthState.Authenticated) {
          loginPendingRef.current = false;
          patchServer({ appState: AppState.Connecting, user: payload.user });
          queryClient.invalidateQueries({ queryKey: [APP_STATE] });
        } else {
          whipRef.current?.stop();
          whipRef.current = null;
          loginPendingRef.current = false;
          setWsStatus(WsStatus.Disconnected);
          queryClient.setQueryData([APP_STATE], serverInitial);
        }
      }),

      onAppState((appState) => patchServer({ appState })),

      onWsStatus((status) => {
        setWsStatus(status);
        if (status === WsStatus.Connected) {
          queryClient.invalidateQueries({ queryKey: [APP_STATE] });
        }
      }),

      onLobbySetup((lobby) =>
        patchServer({ lobby, appState: AppState.StreamSetup }),
      ),

      onLobbyClosed(() => {
        whipRef.current?.stop();
        whipRef.current = null;
        patchServer({
          appState: AppState.Idle,
          lobby: null,
          raceStartAt: null,
        });
      }),

      onLobbyStart((payload) =>
        patchServer({
          raceStartAt: payload.race_start_at,
          appState: AppState.RaceInProgress,
        }),
      ),

      onRaceResults((_results) => {
        whipRef.current?.stop();
        whipRef.current = null;
        patchServer({
          appState: AppState.Finished,
          raceStartAt: null,
        });
      }),
    ];

    return () => unsubs.forEach((fn) => fn());
  }, []);

  async function handleLogin() {
    // We can openLogin only once (refuse if Oauth flow already in progress)
    if (loginPendingRef.current) {
      patchServer({ appState: AppState.Connecting });
      return;
    }
    loginPendingRef.current = true;
    patchServer({ appState: AppState.Connecting });
    try {
      await openLogin();
    } catch (e) {
      const err = e as LoginError;
      if (err.type === LoginErrorType.AlreadyInProgress) return;

      console.error("[auth] open_login error", err.message || err);
      loginPendingRef.current = false;
      patchServer({ appState: AppState.Unauthenticated });
    }
  }

  async function handleLogout() {
    whipRef.current?.stop();
    whipRef.current = null;
    loginPendingRef.current = false;
    setWsStatus(WsStatus.Disconnected);
    queryClient.setQueryData([APP_STATE], serverInitial);
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

  const store: AppStore = { ...serverStore, wsStatus };

  const isConnected = wsStatus === WsStatus.Connected;

  async function handleFinish(finishingTimeMs: number) {
    const lobbyId = serverStore.lobby?.lobby_id;
    if (!lobbyId) return;
    try {
      await sendPlayerFinished(lobbyId, finishingTimeMs);
    } catch (e) {
      console.error("[race] send_player_finished error", e);
    }
  }

  async function handleForfeit() {
    const lobbyId = serverStore.lobby?.lobby_id;
    if (!lobbyId) return;
    whipRef.current?.stop();
    whipRef.current = null;
    try {
      await sendPlayerForfeited(lobbyId);
    } catch (e) {
      console.error("[race] send_player_forfeited error", e);
    }
  }

  async function handleNewRace() {
    try {
      await acknowledgeResults();
    } catch (e) {
      console.error("[race] acknowledge_results error", e);
    }
    patchServer({ appState: AppState.Idle, lobby: null });
  }

  return {
    store,
    isConnected,
    handleLogin,
    handleLogout,
    handleStreamReady,
    handleStopStream,
    handleFinish,
    handleForfeit,
    handleNewRace,
    _patch: patchServer,
  };
}
