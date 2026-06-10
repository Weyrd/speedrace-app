import { useAppState, Phase } from "./store";
import Header from "./components/Header";
import Login from "./components/Login";
import Idle from "./components/Idle";
import StreamSetup from "./components/StreamSetup";
import WaitingForStart from "./components/WaitingForStart";
import Racing from "./components/Racing";
import Finished from "./components/Finished";
import { DevToolbar } from "./store/dev/DevToolbar";
import Footer from "./components/Footer";
import { UpdateChecker } from "./components/UpdateChecker";
import { TrayHint } from "./components/TrayHint";

export default function App() {
  const state = useAppState();

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
