import { useState } from "react";
import { Loader2 } from "lucide-react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import {
  useCaptureSource,
  useEffectiveEncoder,
  useStreamAttempt,
} from "../hooks/useStreamSettings";
import { CaptureSourceKind, EncoderPref, ENCODER_LABELS } from "../types";
import { tryCatch } from "../lib/tryCatch";
import { LobbyHeader } from "./ui/BadgeHelper";
import { SplitList } from "./ui/SplitList";
import { PreviewCanvas } from "./ui/PreviewCanvas";
import { CopyLogsButton } from "./ui/CopyLogsButton";
import SourcePicker from "./SourcePicker";
import { Button } from "./ui/button";

export default function StreamSetup() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation("app");
  const { data: captureSource } = useCaptureSource();
  const { data: effectiveEncoder } = useEffectiveEncoder();
  const { data: streamAttempt } = useStreamAttempt();
  const [error, setError] = useState<string | null>(null);
  const [publishing, setPublishing] = useState(false);
  const [pickerOpen, setPickerOpen] = useState(false);

  const sourceLabel =
    captureSource?.kind === CaptureSourceKind.Window
      ? captureSource.title
      : captureSource
        ? t("stream.monitor_short", { index: captureSource.index + 1 })
        : null;

  const encoderLabel = (id: string) => ENCODER_LABELS[id as EncoderPref] ?? id;

  const handlePublish = async (lobbyId: string) => {
    setError(null);
    setPublishing(true);
    const { error } = await tryCatch(actions.publish(lobbyId));
    setPublishing(false);
    if (error) {
      console.error("[stream] publish_stream error", error);
      setError(
        error instanceof Error ? error.message : t("stream.error_start"),
      );
    }
  };

  if (state.phase !== Phase.StreamSetup) return null;
  const { lobby } = state;

  return (
    <div className="h-full flex flex-col gap-3 px-4 py-4 overflow-y-auto">
      <LobbyHeader
        gameName={lobby.game_name}
        categories={lobby.category_name}
        code={lobby.code}
        label={t("stream.lobby")}
        autosplit={state.autosplit}
        earlyStartDetected={state.autosplit?.run_in_progress}
      />

      <div className="relative shrink-0">
        <PreviewCanvas
          onClick={publishing ? undefined : () => setPickerOpen(true)}
        />
        <CopyLogsButton className="absolute bottom-2 right-2" />
      </div>
      <p className="text-2xs text-dim font-mono tracking-wide text-center -mt-1">
        {t("stream.change_source_hint")}
        {sourceLabel && <span className="text-text"> ({sourceLabel})</span>}
      </p>
      {effectiveEncoder &&
        effectiveEncoder.preferred !== EncoderPref.Auto &&
        effectiveEncoder.preferred !== effectiveEncoder.effective && (
          <p className="text-2xs text-dim font-mono tracking-wide text-center -mt-2">
            {t("stream.encoder_fallback_hint", {
              encoder: encoderLabel(effectiveEncoder.effective),
            })}
          </p>
        )}

      {publishing && streamAttempt && (
        <p className="text-2xs text-dim font-mono tracking-wide text-center -mt-2">
          {t("stream.trying_encoder", { encoder: encoderLabel(streamAttempt) })}
        </p>
      )}

      {lobby.split_resource_updated_at && <SplitList />}

      {error && (
        <p className="text-2xs text-red font-mono tracking-wide leading-relaxed">
          ⚠ {error}
        </p>
      )}

      <Button
        variant="primary"
        size="lg"
        onClick={() => handlePublish(lobby.lobby_id)}
        disabled={publishing}
        className="w-full mt-auto"
      >
        {publishing && <Loader2 size={14} className="animate-spin" />}
        {publishing ? t("stream.publishing") : t("stream.publish")}
      </Button>

      {pickerOpen && <SourcePicker onClose={() => setPickerOpen(false)} />}
    </div>
  );
}
