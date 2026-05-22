import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import StopModal from "./StopModal";
import { LivePill, LobbyBadge } from "./ui/RaceStatus";

export default function WaitingForStart() {
  const state = useAppState();
  const actions = useActions();
  const [showModal, setShowModal] = useState(false);
  const { t } = useTranslation("app");

  if (state.phase !== Phase.WaitingForStart) return null;
  const { lobby } = state;

  return (
    <div className="relative flex flex-col gap-3 px-4 py-4">
      <div className="flex items-center justify-between">
        <LivePill />
        <LobbyBadge id={lobby.lobby_id} />
      </div>

      <div className="flex items-center gap-1.5 bg-green-dim border border-green-dim rounded px-2.5 py-1.5">
        <span className="w-1.5 h-1.5 rounded-full bg-green flex-shrink-0" />
        <span className="text-2xs text-green font-mono tracking-wide">
          {t("stream.stream_active")}
        </span>
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

      {showModal && (
        <StopModal
          RaceInProgress={false}
          onConfirm={() => actions.stopStream()}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}
