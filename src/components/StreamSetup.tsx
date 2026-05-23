import { useState, useRef, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import { WhipClient } from "../stream/whip";
import { LobbyBadge } from "./ui/BadgeHelper";

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
    try {
      const media = await navigator.mediaDevices.getDisplayMedia({
        video: { frameRate: 45 }, // TODO: make configurable
        audio: true,
      });
      streamRef.current = media;
      if (videoRef.current) videoRef.current.srcObject = media;
      media.getVideoTracks()[0].addEventListener("ended", () => {
        setIsPreviewing(false);
        streamRef.current = null;
      });
      setIsPreviewing(true);
    } catch (e) {
      if (e instanceof DOMException && e.name === "NotAllowedError") return;
      setError(
        t("stream.error_source") +
          (e instanceof Error ? ` (${e.message})` : ""),
      );
    }
  }, [stopPreview, t]);

  const handlePublish = useCallback(async () => {
    if (!streamRef.current) return;
    setIsPublishing(true);
    setError(null);
    const client = new WhipClient();
    try {
      await client.start(lobby.whip_url, streamRef.current);
      await actions.streamReady(client, streamRef.current, lobby.lobby_id); // pass stream
      streamRef.current = null;
    } catch (e) {
      console.error("[stream] WHIP publish error", e);
      client.stop();
      setError(e instanceof Error ? e.message : t("stream.error_connection"));
      setIsPublishing(false);
    }
  }, [actions, state, t]);

  if (state.phase !== Phase.StreamSetup) return null;
  const { lobby } = state;

  return (
    <div className="h-full flex flex-col gap-3 px-4 py-4">
      {/* Lobby header */}
      <div className="flex items-center justify-between">
        <span className="text-sm text-muted font-mono tracking-wide">
          {t("stream.lobby")}
        </span>
        <LobbyBadge
          gameName={lobby.game_name}
          categories={lobby.category_name}
        />
      </div>

      {/* Preview area — always clickable to start or change source */}
      <div
        onClick={!isPublishing ? startPreview : undefined}
        className="bg-black border border-border rounded aspect-video w-full flex items-center justify-center overflow-hidden relative group cursor-pointer"
      >
        <video
          ref={videoRef}
          autoPlay
          muted
          className={`w-full h-full object-cover ${isPreviewing ? "" : "hidden"}`}
        />

        {!isPreviewing ? (
          <span className="text-xs text-dim font-mono tracking-wide z-10 group-hover:text-muted transition-colors">
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

      {error && (
        <p className="text-2xs text-red font-mono tracking-wide leading-relaxed">
          ⚠ {error}
        </p>
      )}

      <button
        onClick={handlePublish}
        disabled={!isPreviewing || isPublishing}
        className="w-full py-3.5 text-xs font-mono tracking-wider bg-red text-white rounded border-none cursor-pointer hover:opacity-90 transition-opacity disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {isPublishing ? t("stream.publishing") : t("stream.publish")}
      </button>
    </div>
  );
}
