import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import { formatTime } from "../lib/formatTime";
import { PlayerStatus } from "../types";
import { Button } from "./ui/button";

export default function Finished() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation("app");

  if (state.phase !== Phase.Finished) return null;
  const { result } = state;

  const finished = result.player_status === PlayerStatus.Finished;
  const position = result.finish_position;

  const positionLabel = position
    ? `${position}${["st", "nd", "rd"][position - 1] ?? "th"}`
    : null;

  return (
    <div className="h-full flex flex-col items-center justify-center gap-6 px-6 py-10">
      {/* Position / DNF */}
      <div className="flex flex-col items-center gap-1">
        {finished && positionLabel ? (
          <>
            <span className="text-4xl font-bold font-mono tracking-wide text-text">
              {positionLabel}
            </span>
            <span className="text-2xs text-dim font-mono tracking-wide">
              {t("race.finish_position")}
            </span>
          </>
        ) : (
          <>
            <span className="text-4xl font-bold font-mono tracking-wide text-muted">
              DNF - {positionLabel}
            </span>
            <span className="text-2xs text-dim font-mono tracking-wide">
              {t("race.status_forfeited")}
            </span>
          </>
        )}
      </div>

      {/* Time */}
      {result.finishing_time_ms != null && (
        <div className="flex flex-col items-center gap-1">
          <span
            className={`text-2xl font-bold font-mono tracking-wide ${finished ? "text-text" : "text-muted"}`}
          >
            {formatTime(result.finishing_time_ms)}
          </span>
          <span className="text-2xs text-dim font-mono tracking-wide">
            {t("race.finish_time")}
          </span>
        </div>
      )}

      {/* Status badge */}
      <div
        className={`flex items-center gap-1.5 rounded px-2.5 py-2 border ${
          finished
            ? "bg-green-dim border-green-dim"
            : "bg-red-dim border-red-dim"
        }`}
      >
        <span
          className={`w-1.5 h-1.5 rounded-full shrink-0 ${finished ? "bg-green" : "bg-red"}`}
        />
        <span
          className={`text-2xs font-mono tracking-wide mt-0.5 ${finished ? "text-green" : "text-red"}`}
        >
          {finished ? t("race.status_finished") : t("race.status_forfeited")}
        </span>
      </div>

      <Button variant="outline" onClick={() => actions.newRace()} className="w-full py-3.5 mt-auto">
        {t("race.new_race")}
      </Button>
    </div>
  );
}
