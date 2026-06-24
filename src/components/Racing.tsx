import { useCallback, useEffect, useState } from "react";
import { useSyncExternalStore } from "react";
import { useQuery, useQueryClient } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import StopModal from "./StopModal";
import { LobbyHeader } from "./ui/BadgeHelper";
import { formatTime } from "../lib/formatTime";
import { registerFinishHotkey, unregisterFinishHotkey } from "../lib/commands";
import { useClockOffset } from "../hooks/useClockOffset";
import { playSound, Sound } from "../lib/sound";
import { onSplitLoaded } from "../lib/listeners";

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

const SPLIT_SEGMENTS_KEY = ["split-segments"] as const;
const SPLIT_INDEX_KEY = ["split-current-index"] as const;

export default function Racing() {
  const state = useAppState();
  const actions = useActions();
  const [showModal, setShowModal] = useState(false);
  const queryClient = useQueryClient();
  const { t } = useTranslation("app");

  const { data: splitSegments = [] } = useQuery({
    queryKey: SPLIT_SEGMENTS_KEY,
    queryFn: () => invoke<string[]>("get_split_segments"),
    enabled: false,
    initialData: [],
  });

  const { data: currentSplitIndex = 0 } = useQuery({
    queryKey: SPLIT_INDEX_KEY,
    queryFn: () => invoke<number>("get_current_split_index"),
    enabled: false,
    initialData: 0,
  });
  const now = useSyncExternalStore(subscribeToRaf, getNow);
  const { offsetMs } = useClockOffset();
  const startAt =
    state.phase === Phase.RaceInProgress ? state.raceStartAt : null;

  const videoRef = useCallback(
    (node: HTMLVideoElement | null) => {
      if (node && state.phase === Phase.RaceInProgress) {
        node.srcObject = state.stream;
      }
    },
    [state],
  );

  useEffect(() => {
    registerFinishHotkey().catch((e) =>
      console.error("[race] registerFinishHotkey error", e),
    );
    return () => {
      unregisterFinishHotkey().catch((e) =>
        console.error("[race] unregisterFinishHotkey error", e),
      );
    };
  }, []);

  useEffect(() => {
    const unlisten = onSplitLoaded(() => {
      void queryClient.invalidateQueries({ queryKey: SPLIT_SEGMENTS_KEY });
      void queryClient.invalidateQueries({ queryKey: SPLIT_INDEX_KEY });
    });
    return unlisten;
  }, [queryClient]);

  useEffect(() => {
    if (startAt == null) return;
    const timers = COUNTDOWN_BEEPS.map((b) => {
      const delay = startAt + b.at - offsetMs - Date.now();
      if (delay < 0) return undefined;
      return window.setTimeout(() => playSound(b.sound), delay);
    });
    return () => timers.forEach((t) => t !== undefined && clearTimeout(t));
  }, [startAt, offsetMs]);

  if (state.phase !== Phase.RaceInProgress) return null;
  const { lobby, raceStartAt } = state;

  const elapsed = now + offsetMs - raceStartAt;
  const negative = elapsed < 0;
  const display = (negative ? "-" : "") + formatTime(Math.abs(elapsed));

  const hasSplits =
    lobby.split_resource_updated_at != null && splitSegments.length > 0;
  const hasAutosplitter = lobby.autosplitter_updated_at != null;

  return (
    <div className="h-full flex flex-col gap-3 px-4 py-4">
      <LobbyHeader
        gameName={lobby.game_name}
        categories={lobby.category_name}
        code={lobby.code}
        live
      />
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
      {hasSplits && (
        <div className="flex flex-col items-center gap-0.5">
          <span className="text-xs font-mono text-text">
            {splitSegments[currentSplitIndex]}
          </span>
          <span className="text-2xs font-mono text-dim">
            {currentSplitIndex + 1} / {splitSegments.length}
          </span>
        </div>
      )}
      <div className="flex gap-2 mt-auto">
        {!hasAutosplitter && (
          <button
            onClick={() => actions.finish(lobby.lobby_id, elapsed)}
            disabled={negative}
            className="flex-1 py-3.5 text-xs font-mono tracking-wide border border-green text-green rounded cursor-pointer bg-transparent hover:bg-green-dim transition-colors disabled:opacity-40 disabled:cursor-not-allowed"
          >
            {t("race.finish")}
          </button>
        )}
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
