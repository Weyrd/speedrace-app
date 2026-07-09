import { useState, useRef, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import { WhipClient } from "../stream/whip";
import { tryCatch } from "../lib/tryCatch";
import { LobbyHeader } from "./ui/BadgeHelper";
import { SplitList } from "./ui/SplitList";
import { Button } from "./ui/button";

export default function StreamSetup() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation("app");
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [isPublishing, setIsPublishing] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);

  const stopPreview = useCallback(() => {
    streamRef.current?.getTracks().forEach((tr) => tr.stop());
    streamRef.current = null;
    if (videoRef.current) videoRef.current.srcObject = null;
    setIsPreviewing(false);
  }, []);

  useEffect(() => () => stopPreview(), [stopPreview]);

  const startPreview = useCallback(async () => {
    stopPreview();
    setError(null);
    const { data: media, error } = await tryCatch(
      navigator.mediaDevices.getDisplayMedia({
        video: { frameRate: 50 }, // TODO: v2 make configurable
        audio: true,
      }),
    );
    if (error) {
      if (error instanceof DOMException && error.name === "NotAllowedError")
        return;
      setError(
        t("stream.error_source") +
          (error instanceof Error ? ` (${error.message})` : ""),
      );
      return;
    }
    streamRef.current = media;
    if (videoRef.current) videoRef.current.srcObject = media;
    media.getVideoTracks()[0].addEventListener("ended", () => {
      setIsPreviewing(false);
      streamRef.current = null;
    });
    setIsPreviewing(true);
  }, [stopPreview, t]);

  const handlePublish = useCallback(async () => {
    if (!streamRef.current) return;
    setIsPublishing(true);
    setError(null);
    const client = new WhipClient();
    const { error } = await tryCatch(
      (async () => {
        await client.start(lobby.whip_url, streamRef.current!);
        await actions.streamReady(client, streamRef.current!, lobby.lobby_id); // pass stream
        streamRef.current = null;
      })(),
    );
    if (error) {
      console.error("[stream] WHIP publish error", error);
      client.stop();
      setError(
        error instanceof Error ? error.message : t("stream.error_connection"),
      );
      setIsPublishing(false);
    }
  }, [actions, state, t]);

  if (state.phase !== Phase.StreamSetup) return null;
  const { lobby } = state;

  return (
    <div className="h-full flex flex-col gap-3 px-4 py-4">
      <LobbyHeader
        gameName={lobby.game_name}
        categories={lobby.category_name}
        code={lobby.code}
        label={t("stream.lobby")}
        autosplit={state.autosplit}
        earlyStartDetected={state.autosplit?.run_in_progress}
      />

      {/* Preview area - always clickable to start or change source */}
      <div
        onClick={!isPublishing ? startPreview : undefined}
        className="bg-black border border-border rounded aspect-[1920/1080] w-full flex items-center justify-center overflow-hidden relative group cursor-pointer"
      >
        <video
          ref={videoRef}
          autoPlay
          muted
          className={`w-full h-full object-cover ${isPreviewing ? "" : "hidden"}`}
        />

        {!isPreviewing ? (
          <span className="text-sm text-orange font-mono tracking-wide z-10 group-hover:opacity-80 transition-opacity">
            {t("stream.preview_placeholder")}
          </span>
        ) : (
          <div className="absolute inset-0 bg-black/70 opacity-0 group-hover:opacity-100 transition-opacity flex items-center justify-center z-10">
            <span className="text-xs text-white font-mono tracking-wide">
              {t("stream.click_to_change")}
            </span>
          </div>
        )}
      </div>

      {lobby.split_resource_updated_at && <SplitList />}

      {!isPreviewing && (
        <p className="text-2xs text-dim font-mono tracking-wide leading-relaxed text-center">
          {t("stream.fullscreen_note")}
        </p>
      )}

      {error && (
        <p className="text-2xs text-red font-mono tracking-wide leading-relaxed">
          ⚠ {error}
        </p>
      )}

      <Button
        variant="destructive"
        onClick={handlePublish}
        disabled={!isPreviewing || isPublishing}
        className="w-full py-3.5"
      >
        {isPublishing ? t("stream.publishing") : t("stream.publish")}
      </Button>
    </div>
  );
}
