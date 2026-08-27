import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { WhepClient } from "../../stream/whep";
import {
  EncoderPref,
  ENCODER_LABELS,
  PreviewState,
  StreamStatus,
} from "../../types";
import {
  useEffectiveEncoder,
  useStreamSettings,
} from "../../hooks/useStreamSettings";
import { useStreamPreviewFrame } from "../../hooks/useStreamPreviewFrame";
import { CopyLogsButton } from "./CopyLogsButton";

export function WhepPreview({
  whepUrl,
  streamStatus,
}: {
  whepUrl: string;
  streamStatus: StreamStatus;
}) {
  const videoRef = useRef<HTMLVideoElement>(null);
  const { t } = useTranslation("app");
  const live = streamStatus === StreamStatus.Live;
  const { data: effectiveEncoder } = useEffectiveEncoder();
  const { data: streamSettings } = useStreamSettings();
  const debugStream = streamSettings?.debug_stream ?? false;
  const { status: localPreviewStatus, attachImg: attachLocalPreview } =
    useStreamPreviewFrame();
  const fellBackToEncoder =
    effectiveEncoder &&
    effectiveEncoder.preferred !== EncoderPref.Auto &&
    effectiveEncoder.preferred !== effectiveEncoder.effective
      ? (ENCODER_LABELS[effectiveEncoder.preferred as EncoderPref] ??
        effectiveEncoder.preferred)
      : null;

  useEffect(() => {
    if (!live) return;
    const client = new WhepClient();
    let stopped = false;
    client
      .start(whepUrl)
      .then((stream) => {
        if (!stopped && videoRef.current) videoRef.current.srcObject = stream;
      })
      .catch((e) => console.error("[whep] preview error", e));
    return () => {
      stopped = true;
      client.stop();
    };
  }, [whepUrl, live]);

  return (
    <div className="bg-black border border-border rounded-sm aspect-video w-full shrink-0 overflow-hidden relative">
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        className="w-full h-full object-cover"
      />
      <div className="absolute bottom-2 left-2 flex items-center gap-1.5 bg-black/70 rounded-sm px-2 py-1">
        {live ? (
          <>
            <span className="w-1.5 h-1.5 rounded-full bg-green shrink-0 animate-pulse" />
            <span className="text-2xs text-green font-mono tracking-wide">
              {t("stream.stream_active")}
            </span>
          </>
        ) : (
          <>
            <span className="w-1.5 h-1.5 rounded-full bg-accent shrink-0 animate-pulse" />
            <span className="text-2xs text-accent font-mono tracking-wide">
              {streamStatus === StreamStatus.Reconnecting
                ? t("stream.reconnecting")
                : t("stream.stream_lost")}
            </span>
          </>
        )}
      </div>
      {fellBackToEncoder && (
        <div className="absolute top-2 left-2 bg-black/70 rounded-sm px-2 py-1">
          <span className="text-2xs text-accent font-mono tracking-wide">
            {t("stream.encoder_fallback_hint", { encoder: fellBackToEncoder })}
          </span>
        </div>
      )}
      {debugStream && (
        <div
          className={`absolute top-2 right-2 w-[30%] aspect-video rounded-sm overflow-hidden border border-border/60 bg-black ${
            localPreviewStatus === PreviewState.Live ? "" : "hidden"
          }`}
        >
          <img
            ref={attachLocalPreview}
            alt=""
            className="w-full h-full object-cover"
          />
          <span className="absolute bottom-0.5 left-1 text-3xs font-mono tracking-wide text-white/80 bg-black/50 px-1 rounded-sm">
            {t("stream.local_capture_label")}
          </span>
        </div>
      )}
      {debugStream && (
        <CopyLogsButton className="absolute bottom-2 right-2" />
      )}
    </div>
  );
}
