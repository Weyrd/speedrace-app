import type { User, WsStatus } from "../types";

interface Props {
  user: User | null;
  wsStatus: WsStatus;
  onSettingsClick?: () => void;
}

const WS_DOT: Record<WsStatus, string> = {
  connected:    "bg-green",
  connecting:   "bg-orange animate-pulse",
  disconnected: "bg-dim",
};

const WS_LABEL: Record<WsStatus, string> = {
  connected:    "Connecté",
  connecting:   "Connexion...",
  disconnected: "Déconnecté",
};

export default function Header({ user, wsStatus, onSettingsClick }: Props) {
  return (
    <div className="bg-bg1 px-3 py-2 flex items-center justify-between border-b border-border">
      <span className="flex items-center gap-1.5">
        <span className={`w-1.5 h-1.5 rounded-full ${WS_DOT[wsStatus]}`} />
        <span className="text-2xs text-text font-mono tracking-wide">
          {WS_LABEL[wsStatus]}
        </span>
      </span>
      <span className="text-2xs text-muted font-mono tracking-wide">
        {user?.username ?? "—"}
      </span>
      <button
        onClick={onSettingsClick}
        className="text-dim hover:text-muted transition-colors text-sm cursor-pointer bg-transparent border-none"
      >
        ⚙
      </button>
    </div>
  );
}