import { useEffect, useState } from "react";
import { useSyncExternalStore } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import StopModal from "./StopModal";
import { LobbyHeader } from "./ui/BadgeHelper";
import { SplitList } from "./ui/SplitList";
import { WhepPreview } from "./ui/WhepPreview";
import { formatTime } from "../lib/formatTime";
import { registerFinishHotkey, unregisterFinishHotkey } from "../lib/commands";
import { useFinishHotkey } from "../hooks/useFinishHotkey";
import { useClockOffset } from "../hooks/useClockOffset";
import { primeCountdown, scheduleCountdown, Sound } from "../lib/sound";
import { Button } from "./ui/button";

const COUNTDOWN_BEEPS = [
  { at: -3000, sound: Sound.Countdown3 },
  { at: -2000, sound: Sound.Countdown2 },
  { at: -1000, sound: Sound.Countdown1 },
  { at: 0, sound: Sound.CountdownGo },
] as const;

let rafId: number;
let cachedNow = Date.now();
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
  cachedNow = Date.now();
  clockListeners.forEach((fn) => fn());
  rafId = requestAnimationFrame(tick);
}
function getNow() {
  return cachedNow;
}

export default function Racing() {
  const state = useAppState();
  const actions = useActions();
  const [showModal, setShowModal] = useState(false);
  const { t } = useTranslation("app");

  const now = useSyncExternalStore(subscribeToRaf, getNow);
  const { offsetMs } = useClockOffset();
  const { data: finishHotkey } = useFinishHotkey();
  const startAt =
    state.phase === Phase.RaceInProgress ? state.raceStartAt : null;

  const autosplitDrivesFinish =
    state.phase === Phase.RaceInProgress &&
    (state.lobby.autosplitter_updated_at != null ||
      (state.autosplit?.livesplit === true &&
        state.autosplit.splits_match !== false));

  useEffect(() => {
    if (autosplitDrivesFinish) return;
    registerFinishHotkey().catch((e) =>
      console.error("[race] registerFinishHotkey error", e),
    );
    return () => {
      unregisterFinishHotkey().catch((e) =>
        console.error("[race] unregisterFinishHotkey error", e),
      );
    };
  }, [autosplitDrivesFinish, finishHotkey]);

  useEffect(() => {
    if (startAt == null) return;
    let cancelled = false;
    let cancel: (() => void) | undefined;
    primeCountdown(COUNTDOWN_BEEPS.map((b) => b.sound)).then(() => {
      if (cancelled) return;
      cancel = scheduleCountdown(
        COUNTDOWN_BEEPS.map((b) => ({
          sound: b.sound,
          atMs: startAt + b.at - offsetMs,
        })),
      );
    });
    return () => {
      cancelled = true;
      cancel?.();
    };
  }, [startAt, offsetMs]);

  if (state.phase !== Phase.RaceInProgress) return null;
  const {
    lobby,
    raceStartAt,
    splitIndex,
    completedSegmentTimes,
    currentSegmentStartMs,
  } = state;
  const whepUrl = lobby.whep_url || lobby.whip_url.replace(/\/whip$/, "/whep");

  const elapsed = now + offsetMs - raceStartAt;
  const negative = elapsed < 0;
  const display = (negative ? "-" : "") + formatTime(Math.abs(elapsed));

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
      {lobby.split_resource_updated_at && (
        <SplitList
          currentIndex={splitIndex}
          completedTimes={completedSegmentTimes}
          raceElapsedMs={Math.max(0, elapsed)}
          currentSegmentStartMs={currentSegmentStartMs}
        />
      )}
      <div className="flex gap-2 mt-auto">
        {!autosplitDrivesFinish && (
          <Button
            variant="finish"
            onClick={() => actions.finish(lobby.lobby_id, elapsed)}
            disabled={negative}
            className="flex-1 py-3.5"
          >
            {t("race.finish")}
          </Button>
        )}
        <Button
          variant="forfeit"
          onClick={() => setShowModal(true)}
          className="flex-1 py-3.5"
        >
          {t("race.forfeit")}
        </Button>
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
