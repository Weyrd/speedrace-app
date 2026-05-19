import { useAppState } from "./hooks/useAppState";
import { AppState, type LobbySetup } from "./types";

import Login from "./components/Login";
import Idle from "./components/Idle";
import StreamSetup from "./components/StreamSetup";
import WaitingForStart from "./components/WaitingForStart";
import Racing from "./components/Racing";

const IS_DEV = import.meta.env.DEV;

const DEV_STATES = Object.values(AppState).filter(
  s => s !== AppState.Connecting && s !== AppState.Finished
) satisfies AppState[];

const MOCK_LOBBY: LobbySetup = {
  lobby_id: "SMB-4821",
  stream_key: "mock-key",
  whip_url: "https://stream.momentum.weyrd.space:8889/mystream/whip",
  game_name: "Mock Game",
  category_name: ["Celeste", "Any%"],
};

export default function App() {
  const {
    store,
    handleLogin,
    handleLogout,
    handleStreamReady,
    handleStopStream,
    _patch,
  } = useAppState();

  function renderScreen() {
    switch (store.appState) {
      case AppState.Unauthenticated:
      case AppState.Connecting:
        return (
          <Login
            onLogin={handleLogin}
            isConnecting={store.appState === AppState.Connecting}
          />
        );

      case AppState.Idle:
        return (
          <Idle user={store.user} wsStatus={store.wsStatus} onLogout={handleLogout} />
        );

      case AppState.StreamSetup:
        return (
          <StreamSetup
            user={store.user}
            wsStatus={store.wsStatus}
            lobby={store.lobby!}
            onStreamReady={handleStreamReady}
            onLogout={handleLogout}
          />
        );

      case AppState.WaitingForStart:
        return (
          <WaitingForStart
            user={store.user}
            wsStatus={store.wsStatus}
            lobby={store.lobby!}
            onStop={handleStopStream}
          />
        );

      case AppState.Racing:
      case AppState.Finished:
        return (
          <Racing
            user={store.user}
            wsStatus={store.wsStatus}
            lobby={store.lobby!}
            raceStartAt={store.raceStartAt!}
            onStop={handleStopStream}
          />
        );

      default:
        return null;
    }
  }

  return (
    <div className="min-h-screen bg-bg0 flex flex-col">
      {renderScreen()}

      {IS_DEV && (
        <DevToolbar
          current={store.appState}
          onState={s => {
            switch (s) {
              case AppState.StreamSetup:
                _patch({ appState: AppState.StreamSetup, lobby: MOCK_LOBBY });
                break;
              case AppState.WaitingForStart:
                _patch({ appState: AppState.WaitingForStart, lobby: MOCK_LOBBY });
                break;
              case AppState.Racing:
                _patch({
                  appState: AppState.Racing,
                  lobby: MOCK_LOBBY,
                  raceStartAt: new Date(Date.now() + 3000).toISOString(),
                });
                break;
              default:
                _patch({ appState: s });
            }
          }}
        />
      )}
    </div>
  );
}

function DevToolbar({
  current,
  onState,
}: {
  current: AppState;
  onState: (s: AppState) => void;
}) {
  return (
    <div className="fixed bottom-0 left-0 right-0 bg-bg1 border-t border-border px-2 py-1 flex gap-1 flex-wrap z-50">
      <span className="text-2xs text-dim font-mono tracking-wide self-center mr-1">DEV</span>
      {DEV_STATES.map(s => (
        <button
          key={s}
          onClick={() => onState(s)}
          className={`
            text-2xs font-mono tracking-wide px-2 py-0.5 rounded cursor-pointer border transition-colors
            ${current === s
              ? "bg-orange text-white border-orange"
              : "bg-transparent text-muted border-border hover:border-muted hover:text-text"
            }
          `}
        >
          {s}
        </button>
      ))}
    </div>
  );
}