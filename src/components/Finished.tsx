import { FolderOpen } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import { formatTime } from "../lib/formatTime";
import { PlayerStatus, RaceType, UploadPhase } from "../types";
import { openReplayDir } from "../lib/commands";
import { useStreamSettings } from "../hooks/useStreamSettings";
import { Button } from "./ui/button";

const UPLOAD_ACTIVE = new Set<UploadPhase>([
  UploadPhase.Preparing,
  UploadPhase.Uploading,
  UploadPhase.Processing,
]);

export default function Finished() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation("app");
  const { data: streamSettings } = useStreamSettings();

  if (state.phase !== Phase.Finished) return null;
  const { result } = state;

  const finished = result.player_status === PlayerStatus.Finished;
  const position = result.finish_position;

  const positionLabel = position
    ? `${position}${["st", "nd", "rd"][position - 1] ?? "th"}`
    : null;

  const replaySaved =
    state.raceType === RaceType.Ranked ||
    (state.raceType === RaceType.Casual &&
      (streamSettings?.replay_casual ?? false));

  const upload = state.upload;
  const uploadActive = upload != null && UPLOAD_ACTIVE.has(upload.state);
  const uploadPct =
    upload && upload.total_bytes > 0
      ? Math.min(100, (upload.uploaded_bytes / upload.total_bytes) * 100)
      : 0;
  const uploadRetryable =
    upload != null &&
    (upload.state === UploadPhase.Failed ||
      upload.state === UploadPhase.QuotaExhausted ||
      upload.state === UploadPhase.Abandoned);

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

      {upload && (
        <div className="flex w-full flex-col items-center gap-2">
          <span
            className={`text-2xs font-mono tracking-wide ${
              upload.state === UploadPhase.Done
                ? "text-green"
                : uploadRetryable
                  ? "text-red"
                  : "text-dim"
            }`}
          >
            {t(`upload.${upload.state}`)}
          </span>
          {upload.state === UploadPhase.Uploading && (
            <div className="h-1.5 w-full rounded-sm bg-surface overflow-hidden">
              <div
                className="h-full rounded-sm bg-green transition-[width] duration-300"
                style={{ width: `${uploadPct}%` }}
              />
            </div>
          )}
          {uploadActive && (
            <>
              <span className="text-2xs font-mono tracking-wide text-dim">
                {t("upload.blocking")}
              </span>
              <Button
                variant="outline"
                onClick={() => void actions.abandonUpload()}
                className="w-full py-2"
              >
                {t("upload.abandon")}
              </Button>
            </>
          )}
          {uploadRetryable && (
            <Button
              variant="outline"
              onClick={() => void actions.retryUpload()}
              className="w-full py-2"
            >
              {t("upload.retry")}
            </Button>
          )}
        </div>
      )}

      {replaySaved && (
        <div className="mt-auto flex w-full flex-col items-center gap-2">
          <span className="text-2xs font-mono tracking-wide text-dim">
            {t("replay.saved")}
          </span>
          <Button
            variant="outline"
            onClick={() => void openReplayDir()}
            className="w-full py-3"
          >
            <FolderOpen size={14} />
            {t("replay.show_in_folder")}
          </Button>
        </div>
      )}

      <Button
        variant="outline"
        onClick={() => actions.newRace()}
        disabled={uploadActive}
        className={`w-full py-3.5 ${replaySaved ? "" : "mt-auto"}`}
      >
        {t("race.new_race")}
      </Button>
    </div>
  );
}
