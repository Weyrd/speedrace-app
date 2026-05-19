import Header from "../components/Header";
import TitleBar from "../components/TitleBar";
import type { User, WsStatus } from "../types";

interface Props {
  user: User | null;
  wsStatus: WsStatus;
  onLogout: () => void;
}

export default function Idle({ user, wsStatus, onLogout }: Props) {
  return (
    <div className="flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <Header user={user} wsStatus={wsStatus} onSettingsClick={onLogout} />
      <div className="flex flex-col items-center justify-center gap-2 px-3 py-8 text-center">
        <span className="text-3xl text-dim">⏳</span>
        <p className="text-xs text-text font-mono tracking-wide font-bold">En attente</p>
        <p className="text-2xs text-dim font-mono tracking-wide leading-relaxed">
          Rejoins un lobby sur<br />le web pour commencer.
        </p>
      </div>
    </div>
  );
}