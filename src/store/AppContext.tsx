import {
  createContext,
  useContext,
  useReducer,
  useRef,
  useEffect,
  type Dispatch,
  type MutableRefObject,
  type ReactNode,
} from "react";
import { appReducer, initialState } from "./appReducer";
import { ActionType, type AppState, type AppAction } from "./types";
import { AppEventBridge } from "./AppEventBridge";
import { getCurrentUser, getLobbyState } from "../lib/commands";
import type { WhipClient } from "../stream/whip";

const AppStateContext = createContext<AppState>(initialState);
const AppDispatchContext = createContext<Dispatch<AppAction>>(() => {});
const WhipRefContext = createContext<MutableRefObject<WhipClient | null>>({
  current: null,
});

export function useAppState(): AppState {
  return useContext(AppStateContext);
}

export function useAppDispatch(): Dispatch<AppAction> {
  return useContext(AppDispatchContext);
}

export function useWhipRef(): MutableRefObject<WhipClient | null> {
  return useContext(WhipRefContext);
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(appReducer, initialState);
  const whipRef = useRef<WhipClient | null>(null);

  useEffect(() => {
    let cancelled = false;

    async function hydrate() {
      try {
        const [user, clientState] = await Promise.all([
          getCurrentUser(),
          getLobbyState(),
        ]);

        if (cancelled) return;

        if (user) {
          dispatch({ type: ActionType.AuthOk, user });
          if (clientState.lobby) {
            dispatch({
              type: ActionType.LobbySetup,
              lobby: clientState.lobby,
            });
            dispatch({
              type: ActionType.AutosplitStatus,
              status: clientState.autosplit,
            });
          }
        }
      } catch {
        // App starts unauthenticated
      }
    }

    hydrate();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <WhipRefContext.Provider value={whipRef}>
      <AppDispatchContext.Provider value={dispatch}>
        <AppStateContext.Provider value={state}>
          <AppEventBridge />
          {children}
        </AppStateContext.Provider>
      </AppDispatchContext.Provider>
    </WhipRefContext.Provider>
  );
}
