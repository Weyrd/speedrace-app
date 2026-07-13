import { useEffect, useRef } from "react";
import { useTranslation } from "react-i18next";
import { WhepClient } from "../../stream/whep";
import { StreamStatus } from "../../types";

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
    <div className="bg-black border border-border rounded aspect-1920/1080 w-full overflow-hidden relative">
      <video
        ref={videoRef}
        autoPlay
        muted
        playsInline
        className="w-full h-full object-cover"
      />
      <div className="absolute bottom-2 left-2 flex items-center gap-1.5 bg-black/70 rounded px-2 py-1">
        {live ? (
          <>
            <span className="w-1.5 h-1.5 rounded-full bg-green shrink-0 animate-pulse" />
            <span className="text-2xs text-green font-mono tracking-wide">
              {t("stream.stream_active")}
            </span>
          </>
        ) : (
          <>
            <span className="w-1.5 h-1.5 rounded-full bg-orange shrink-0 animate-pulse" />
            <span className="text-2xs text-orange font-mono tracking-wide">
              {streamStatus === StreamStatus.Reconnecting
                ? t("stream.reconnecting")
                : t("stream.stream_lost")}
            </span>
          </>
        )}
      </div>
    </div>
  );
}
