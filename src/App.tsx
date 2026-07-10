import { useEffect } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { LogicalSize } from "@tauri-apps/api/dpi";
import { useAppState, Phase } from "./store";
import Header from "./components/Header";
import Login from "./components/Login";
import Idle from "./components/Idle";
import StreamSetup from "./components/StreamSetup";
import WaitingForStart from "./components/WaitingForStart";
import Racing from "./components/Racing";
import Finished from "./components/Finished";
import ServerUnavailable from "./components/ServerUnavailable";
import Banned from "./components/Banned";
import { DevToolbar } from "./store/dev/DevToolbar";
import Footer from "./components/Footer";
import { UpdateChecker } from "./components/UpdateChecker";
import { TrayHint } from "./components/TrayHint";

const WIN_TALL = new LogicalSize(400, 740);
const WIN_NORMAL = new LogicalSize(400, 600);

function hasSplits(state: ReturnType<typeof useAppState>): boolean {
  return (
    (state.phase === Phase.StreamSetup ||
      state.phase === Phase.WaitingForStart ||
      state.phase === Phase.RaceInProgress) &&
    state.lobby.split_resource_updated_at != null
  );
}

export default function App() {
  const state = useAppState();
  const tall = hasSplits(state);

  useEffect(() => {
    void (async () => {
      const win = getCurrentWindow();
      if (await win.isFullscreen()) return;
      win
        .setSize(tall ? WIN_TALL : WIN_NORMAL)
        .catch((e) => console.error("[window] setSize failed:", e));
    })();
  }, [tall]);

  function renderScreen() {
    switch (state.phase) {
      case Phase.Unauthenticated:
      case Phase.Connecting:
        return <Login />;
      case Phase.Idle:
        return <Idle />;
      case Phase.StreamSetup:
        return <StreamSetup />;
      case Phase.WaitingForStart:
        return <WaitingForStart />;
      case Phase.RaceInProgress:
        return <Racing />;
      case Phase.Finished:
        return <Finished />;
      case Phase.ServerUnavailable:
        return <ServerUnavailable />;
      case Phase.Banned:
        return <Banned />;
      default:
        return null;
    }
  }

  return (
    <div className="h-screen bg-bg0 flex flex-col">
      <Header />
      <div className="flex-1 min-h-0 overflow-hidden">{renderScreen()}</div>
      <Footer />
      <UpdateChecker />
      <TrayHint />
      {import.meta.env.DEV && <DevToolbar />}
    </div>
  );
}
