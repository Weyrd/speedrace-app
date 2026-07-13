import { useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { onStreamPreview } from "../../lib/listeners";
import { PreviewState } from "../../types";

export function PreviewCanvas({ onClick }: { onClick?: () => void }) {
  const { t } = useTranslation("app");
  const [status, setStatus] = useState<PreviewState>(PreviewState.Starting);
  const statusRef = useRef(status);
  statusRef.current = status;
  const unlistenRef = useRef<(() => void) | null>(null);

  const attachImg = useCallback((node: HTMLImageElement | null) => {
    unlistenRef.current?.();
    unlistenRef.current = null;
    if (!node) return;
    unlistenRef.current = onStreamPreview((p) => {
      if (p.frame) {
        node.src = `data:image/jpeg;base64,${p.frame}`;
        if (statusRef.current !== PreviewState.Live)
          setStatus(PreviewState.Live);
      } else if (p.error) {
        if (statusRef.current !== PreviewState.Error)
          setStatus(PreviewState.Error);
      }
    });
  }, []);

  return (
    <div
      onClick={onClick}
      className={`bg-black border border-border rounded aspect-1920/1080 w-full overflow-hidden relative ${onClick ? "cursor-pointer" : ""}`}
    >
      <img
        ref={attachImg}
        alt=""
        className={`w-full h-full object-contain ${status === PreviewState.Live ? "" : "hidden"}`}
      />
      {status !== PreviewState.Live && (
        <div className="absolute inset-0 flex items-center justify-center">
          <span
            className={`text-sm font-mono tracking-wide ${status === PreviewState.Error ? "text-red" : "text-orange"}`}
          >
            {status === PreviewState.Error
              ? t("stream.preview_error")
              : t("stream.preview_starting")}
          </span>
        </div>
      )}
    </div>
  );
}
