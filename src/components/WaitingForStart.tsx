import { useRef, useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import StopModal from "./StopModal";
import { LivePill, LobbyBadge } from "./ui/BadgeHelper";

export default function WaitingForStart() {
  const state = useAppState();
  const actions = useActions();
  const [showModal, setShowModal] = useState(false);
  const { t } = useTranslation("app");
  const videoRef = useRef<HTMLVideoElement>(null);

  useEffect(() => {
    if (state.phase !== Phase.WaitingForStart) return;
    if (videoRef.current && state.stream) {
      videoRef.current.srcObject = state.stream;
    }
  }, [state]);

  if (state.phase !== Phase.WaitingForStart) return null;
  const { lobby } = state;

  return (
    <div className="h-full flex flex-col gap-3 px-4 py-4">
      {/* Header */}
      <div className="flex items-center justify-between">
        <LivePill />
        <LobbyBadge id={lobby.lobby_id} />
      </div>

      {/* Stream preview — view only */}
      <div className="bg-black border border-border rounded aspect-video w-full overflow-hidden relative">
        <video
          ref={videoRef}
          autoPlay
          muted
          className="w-full h-full object-cover"
        />
        {/* Stream active badge overlaid on preview */}
        <div className="absolute bottom-2 left-2 flex items-center gap-1.5 bg-black/70 rounded px-2 py-1">
          <span className="w-1.5 h-1.5 rounded-full bg-green shrink-0 animate-pulse" />
          <span className="text-2xs text-green font-mono tracking-wide">
            {t("stream.stream_active")}
          </span>
        </div>
      </div>

      {/* Waiting message */}
      <p className="text-2xs text-dim font-mono tracking-wide text-center leading-relaxed whitespace-pre-line">
        {t("waiting.waiting_host")}
      </p>

      <button
        onClick={() => setShowModal(true)}
        className="w-full py-3.5 text-xs font-mono tracking-wide border border-dim text-muted rounded cursor-pointer bg-transparent hover:border-muted transition-colors mt-auto"
      >
        {t("stream.stop_stream")}
      </button>

      {showModal && (
        <StopModal
          raceInProgress={false}
          onConfirm={() => actions.stopStream()}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}
