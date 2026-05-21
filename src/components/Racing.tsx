// Pas de useEffect pour le timer — on utilise useSyncExternalStore + Date.now()
// Pattern : le composant se re-render via requestAnimationFrame, pas via un interval dans un effect.
import { useState } from "react";
import { useSyncExternalStore } from "react";
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
  raceStartAt: number;
  onStop: () => void;
  onLogout: () => void;
}

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
  clockListeners.forEach(fn => fn());
  rafId = requestAnimationFrame(tick);
}

function getNow() { return Date.now(); }

export default function Racing({ user, wsStatus, lobby, raceStartAt, onStop, onLogout }: Props) {
  const [showModal, setShowModal] = useState(false);
  const { t } = useTranslation("app");
  const now = useSyncExternalStore(subscribeToRaf, getNow);
  const elapsed = Math.max(0, now - raceStartAt);
  const h = Math.floor(elapsed / 3_600_000);
  const m = Math.floor((elapsed % 3_600_000) / 60_000);
  const s = Math.floor((elapsed % 60_000) / 1000);
  const display = h > 0
    ? `${String(h).padStart(2, "0")}:${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`
    : `${String(m).padStart(2, "0")}:${String(s).padStart(2, "0")}`;

  return (
    <div className="relative flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <Header user={user} wsStatus={wsStatus} onSettingsClick={onLogout} />
      <div className="px-3 py-3.5 flex flex-col gap-2.5">

        <div className="flex items-center justify-between">
          <LivePill />
          <LobbyBadge id={lobby.lobby_id} />
        </div>

        <div className="flex flex-col items-center py-3 gap-1">
          <span className="text-4xl font-bold font-mono tracking-wide text-text">
            {display}
          </span>
          <span className="text-2xs text-muted font-mono tracking-wide">{t("race.in_race")}</span>
        </div>

        <button
          onClick={() => setShowModal(true)}
          className="w-full py-2 text-2xs font-mono tracking-wide border border-red text-red rounded cursor-pointer bg-transparent hover:bg-red-dim transition-colors"
        >
          {t("race.stop_forfeit")}
        </button>
      </div>

      {showModal && (
        <StopModal
          isRacing={true}
          onConfirm={onStop}
          onCancel={() => setShowModal(false)}
        />
      )}
    </div>
  );
}
