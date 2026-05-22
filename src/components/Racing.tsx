import { useState, useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import StopModal from "./StopModal";
import { LivePill, LobbyBadge } from "./ui/RaceStatus";

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

  if (state.phase !== Phase.RaceInProgress) return null;
  const { lobby, raceStartAt } = state;

  const elapsed = Math.max(0, now - raceStartAt);
  const h = Math.floor(elapsed / 3_600_000);
  const m = Math.floor((elapsed % 3_600_000) / 60_000);
  const s = Math.floor((elapsed % 60_000) / 1000);
  const display =
    h > 0
      ? `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
      : `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;

  return (
    <div className="relative flex flex-col gap-3 px-4 py-4">
      <div className="flex items-center justify-between">
        <LivePill />
        <LobbyBadge id={lobby.lobby_id} />
      </div>

      <div className="flex flex-col items-center py-4 gap-1">
        <span className="text-4xl font-bold font-mono tracking-wide text-text">
          {display}
        </span>
        <span className="text-2xs text-muted font-mono tracking-wide">
          {t("race.in_race")}
        </span>
      </div>

      <div className="flex gap-2">
        <button
          onClick={() => actions.finish(lobby.lobby_id, elapsed)}
          className="flex-1 py-2 text-2xs font-mono tracking-wide border border-green text-green rounded cursor-pointer bg-transparent hover:bg-green-dim transition-colors"
        >
          {t("race.finish")}
        </button>
        <button
          onClick={() => setShowModal(true)}
          className="flex-1 py-2 text-2xs font-mono tracking-wide border border-red text-red rounded cursor-pointer bg-transparent hover:bg-red-dim transition-colors"
        >
          {t("race.forfeit")}
        </button>
      </div>

      {showModal && (
        <StopModal
          RaceInProgress={true}
          onConfirm={() => actions.forfeit(lobby.lobby_id)}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}
