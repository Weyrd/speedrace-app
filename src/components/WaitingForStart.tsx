import { useState } from "react";
import Header from "../components/Header";
import TitleBar from "../components/TitleBar";
import StopModal from "../components/StopModal";
import type { User, LobbySetup, WsStatus } from "../types";

interface Props {
  user: User | null;
  wsStatus: WsStatus;
  lobby: LobbySetup;
  onStop: () => void;
}

export default function WaitingForStart({ user, wsStatus, lobby, onStop }: Props) {
  const [showModal, setShowModal] = useState(false);

  return (
    <div className="relative flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <Header user={user} wsStatus={wsStatus} />
      <div className="px-3 py-3.5 flex flex-col gap-2.5">

        {/* Status row */}
        <div className="flex items-center justify-between">
          <LivePill />
          <LobbyBadge id={lobby.lobby_id} />
        </div>

        {/* Stream ok */}
        <div className="flex items-center gap-1.5 bg-green-dim border border-green-dim rounded px-2.5 py-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-green flex-shrink-0" />
          <span className="text-2xs text-green font-mono tracking-wide">Stream actif</span>
        </div>

        <p className="text-2xs text-dim font-mono tracking-wide text-center leading-relaxed">
          En attente que l'hôte<br />lance la race...
        </p>

        <button
          onClick={() => setShowModal(true)}
          className="w-full py-2 text-2xs font-mono tracking-wide border border-dim text-muted rounded cursor-pointer bg-transparent hover:border-muted transition-colors"
        >
          ▪ Arrêter le stream
        </button>
      </div>

      {showModal && (
        <StopModal
          isRacing={false}
          onConfirm={onStop}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}

// ─── Shared mini-components ───────────────────────────────────────────────────

export function LivePill() {
  return (
    <span className="inline-flex items-center gap-1.5 bg-red/15 border border-red/40 rounded px-2 py-0.5 text-2xs font-mono tracking-wider text-red font-bold">
      <span className="w-1.5 h-1.5 rounded-full bg-red animate-pulse" />
      LIVE
    </span>
  );
}

export function LobbyBadge({ id }: { id: string }) {
  return (
    <span className="bg-bg2 border border-border rounded px-2 py-0.5 text-2xs font-mono tracking-wide text-muted">
      <span className="text-orange">{id}</span>
    </span>
  );
}