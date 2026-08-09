import { useCallback, useRef, useState } from "react";
import { useTranslation } from "react-i18next";
import { onStreamPreview } from "../../lib/listeners";
import { PreviewState } from "../../types";
import { Button } from "./button";

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
    <Button
      type="button"
      variant="ghost"
      onClick={onClick}
      disabled={!onClick}
      aria-label={t("stream.change_source_hint")}
      className="relative aspect-video w-full overflow-hidden rounded-sm border border-border bg-black p-0"
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
    </Button>
  );
}
