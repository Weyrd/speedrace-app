import { useState } from "react";
import { useTranslation } from "react-i18next";
import Header from "../components/Header";
import TitleBar from "../components/TitleBar";
import StopModal from "../components/StopModal";
import { LivePill, LobbyBadge } from "../components/ui/RaceStatus";
import type { User, LobbySetup, WsStatus } from "../types";

interface Props {
  user: User | null;
  wsStatus: WsStatus;
  lobby: LobbySetup;
  onStop: () => void;
  onLogout: () => void;
}

export default function WaitingForStart({ user, wsStatus, lobby, onStop, onLogout }: Props) {
  const [showModal, setShowModal] = useState(false);
  const { t } = useTranslation("app");

  return (
    <div className="relative flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <Header user={user} wsStatus={wsStatus} onSettingsClick={onLogout} />
      <div className="px-3 py-3.5 flex flex-col gap-2.5">

        {/* Status row */}
        <div className="flex items-center justify-between">
          <LivePill />
          <LobbyBadge id={lobby.lobby_id} />
        </div>

        {/* Stream ok */}
        <div className="flex items-center gap-1.5 bg-green-dim border border-green-dim rounded px-2.5 py-1.5">
          <span className="w-1.5 h-1.5 rounded-full bg-green flex-shrink-0" />
          <span className="text-2xs text-green font-mono tracking-wide">{t("stream.stream_active")}</span>
        </div>

        <p className="text-2xs text-dim font-mono tracking-wide text-center leading-relaxed whitespace-pre-line">
          {t("waiting.waiting_host")}
        </p>

        <button
          onClick={() => setShowModal(true)}
          className="w-full py-2 text-2xs font-mono tracking-wide border border-dim text-muted rounded cursor-pointer bg-transparent hover:border-muted transition-colors"
        >
          {t("stream.stop_stream")}
        </button>
      </div>

      {showModal && (
        <StopModal
          RaceInProgress={false}
          onConfirm={onStop}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}
