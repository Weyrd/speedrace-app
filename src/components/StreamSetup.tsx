import { useState, useRef, useCallback, useEffect } from "react";
import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import { WhipClient } from "../stream/whip";

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
        video: { frameRate: 30 },
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

  if (state.phase !== Phase.StreamSetup) return null;
  const { lobby } = state;

  const handlePublish = async () => {
    if (!streamRef.current) return;
    setIsPublishing(true);
    setError(null);

    const client = new WhipClient();
    try {
      await client.start(lobby.whip_url, streamRef.current);
      streamRef.current = null;
      await actions.streamReady(client, lobby.lobby_id);
    } catch (e) {
      console.error("[stream] WHIP publish error", e);
      client.stop();
      setError(e instanceof Error ? e.message : t("stream.error_connection"));
      setIsPublishing(false);
    }
  };

  return (
    <div className="flex flex-col gap-3 px-4 py-4">
      <div className="flex items-center justify-between">
        <span className="text-2xs text-muted font-mono tracking-wide">
          {t("stream.lobby")}
        </span>
        <span className="bg-bg2 border border-border rounded px-2 py-0.5 text-2xs font-mono tracking-wide text-orange">
          {lobby.lobby_id}
        </span>
      </div>

      <div
        onClick={!isPreviewing ? startPreview : undefined}
        className="bg-black border border-border rounded h-20 flex items-center justify-center overflow-hidden relative cursor-pointer group"
      >
        <video
          ref={videoRef}
          autoPlay
          muted
          className={`w-full h-full object-cover ${isPreviewing ? "" : "hidden"}`}
        />
        {!isPreviewing && (
          <span className="text-2xs text-dim font-mono tracking-wide z-10 group-hover:text-muted transition-colors">
            {t("stream.preview_placeholder")}
          </span>
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
        className="w-full py-2 text-2xs font-mono tracking-wider bg-red text-white rounded border-none cursor-pointer hover:opacity-90 transition-opacity disabled:opacity-40 disabled:cursor-not-allowed"
      >
        {isPublishing ? t("stream.publishing") : t("stream.publish")}
      </button>
    </div>
  );
}
