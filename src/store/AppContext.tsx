import {
  createContext,
  useContext,
  useReducer,
  useEffect,
  type Dispatch,
  type ReactNode,
} from "react";
import { appReducer, initialState } from "./appReducer";
import { ActionType, type AppState, type AppAction } from "./types";
import { AppEventBridge } from "./AppEventBridge";
import { getCurrentUser, getLobbyState } from "../lib/commands";
import { tryCatch } from "../lib/tryCatch";

const AppStateContext = createContext<AppState>(initialState);
const AppDispatchContext = createContext<Dispatch<AppAction>>(() => {});

export function useAppState(): AppState {
  return useContext(AppStateContext);
}

export function useAppDispatch(): Dispatch<AppAction> {
  return useContext(AppDispatchContext);
}

export function AppProvider({ children }: { children: ReactNode }) {
  const [state, dispatch] = useReducer(appReducer, initialState);

  useEffect(() => {
    let cancelled = false;

    async function hydrate() {
      const { data, error } = await tryCatch(
        Promise.all([getCurrentUser(), getLobbyState()]),
      );
      if (error) return; // App starts unauthenticated
      if (cancelled) return;

      const [user, clientState] = data;
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
    }

    hydrate();
    return () => {
      cancelled = true;
    };
  }, []);

  return (
    <AppDispatchContext.Provider value={dispatch}>
      <AppStateContext.Provider value={state}>
        <AppEventBridge />
        {children}
      </AppStateContext.Provider>
    </AppDispatchContext.Provider>
  );
}
