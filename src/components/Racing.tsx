import { useCallback, useState } from "react";
import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import StopModal from "./StopModal";
import { LivePill, LobbyBadge } from "./ui/RaceStatus";
import { formatTime } from "../lib/formatTime";

let rafId: number;
const clockListeners = new Set<() => void>();
function subscribeToRaf(cb: () => void) {
  clockListeners.add(cb);
  if (clockListeners.size === 1) tick();
  return () => {
    clockListeners.delete(cb);
    if (clockListeners.size === 0) cancelAnimationFrame(rafId);
  };
}
function tick() {
  clockListeners.forEach((fn) => fn());
  rafId = requestAnimationFrame(tick);
}
function getNow() {
  return Date.now();
}

export default function Racing() {
  const state = useAppState();
  const actions = useActions();
  const [showModal, setShowModal] = useState(false);
  const { t } = useTranslation("app");
  const now = useSyncExternalStore(subscribeToRaf, getNow);

  const videoRef = useCallback(
    (node: HTMLVideoElement | null) => {
      if (node && state.phase === Phase.RaceInProgress) {
        node.srcObject = state.stream;
      }
    },
    [state],
  );

  if (state.phase !== Phase.RaceInProgress) return null;
  const { lobby, raceStartAt } = state;

  const elapsed = now - raceStartAt;
  const negative = elapsed < 0;
  const display = (negative ? "-" : "") + formatTime(Math.abs(elapsed));

  return (
    <div className="h-full flex flex-col gap-3 px-4 py-4">
      <div className="flex items-center justify-between">
        <LivePill />
        <LobbyBadge id={lobby.lobby_id} />
      </div>
      <div className="bg-black border border-border rounded aspect-video w-full overflow-hidden relative">
        <video
          ref={videoRef}
          autoPlay
          muted
          className="w-full h-full object-cover"
        />
        <div className="absolute bottom-2 left-2 flex items-center gap-1.5 bg-black/70 rounded px-2 py-1">
          <span className="w-1.5 h-1.5 rounded-full bg-green shrink-0 animate-pulse" />
          <span className="text-2xs text-green font-mono tracking-wide">
            {t("stream.stream_active")}
          </span>
        </div>
      </div>
      <div className="flex flex-col items-center py-2 gap-1">
        <span
          className={`text-4xl font-bold font-mono tracking-wide transition-colors ${negative ? "text-muted" : "text-text"}`}
        >
          {display}
        </span>
        <span className="text-2xs text-dim font-mono tracking-wide">
          {negative ? t("race.starting_soon") : t("race.in_race")}
        </span>
      </div>
      <div className="flex gap-2 mt-auto">
        <button
          onClick={() => actions.finish(lobby.lobby_id, elapsed)}
          disabled={negative}
          className="flex-1 py-3.5 text-xs font-mono tracking-wide border border-green text-green rounded cursor-pointer bg-transparent hover:bg-green-dim transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {t("race.finish")}
        </button>
        <button
          onClick={() => setShowModal(true)}
          className="flex-1 py-3.5 text-xs font-mono tracking-wide border border-red text-red rounded cursor-pointer bg-transparent hover:bg-red-dim transition-colors"
        >
          {t("race.forfeit")}
        </button>
      </div>
      {showModal && (
        <StopModal
          raceInProgress={true}
          onConfirm={() => actions.forfeit(lobby.lobby_id)}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}
