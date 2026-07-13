import { useState } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import StopModal from "./StopModal";
import { LobbyHeader } from "./ui/BadgeHelper";
import { SplitList } from "./ui/SplitList";
import { WhepPreview } from "./ui/WhepPreview";
import { Button } from "./ui/button";

export default function WaitingForStart() {
  const state = useAppState();
  const actions = useActions();
  const [showModal, setShowModal] = useState(false);
  const { t } = useTranslation("app");

  if (state.phase !== Phase.WaitingForStart) return null;
  const { lobby } = state;
  const whepUrl = lobby.whep_url || lobby.whip_url.replace(/\/whip$/, "/whep");

  return (
    <div className="h-full flex flex-col gap-3 px-4 py-4">
      <LobbyHeader
        gameName={lobby.game_name}
        categories={lobby.category_name}
        code={lobby.code}
        live
        autosplit={state.autosplit}
        earlyStartDetected={state.autosplit?.run_in_progress}
      />

      <WhepPreview whepUrl={whepUrl} streamStatus={state.streamStatus} />

      {lobby.split_resource_updated_at && <SplitList />}

      {/* Waiting message */}
      <p className="text-2xs text-dim font-mono tracking-wide text-center leading-relaxed whitespace-pre-line">
        {t("waiting.waiting_host")}
      </p>

      <Button
        variant="outline"
        onClick={() => setShowModal(true)}
        className="w-full py-3.5 mt-auto border-dim"
      >
        {t("stream.stop_stream")}
      </Button>

      {showModal && (
        <StopModal
          raceInProgress={false}
          onConfirm={() => actions.stopStream(lobby.lobby_id)}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}
