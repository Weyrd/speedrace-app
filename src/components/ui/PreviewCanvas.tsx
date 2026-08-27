import { useTranslation } from "react-i18next";
import { useStreamPreviewFrame } from "../../hooks/useStreamPreviewFrame";
import { PreviewState } from "../../types";
import { Button } from "./button";

export function PreviewCanvas({ onClick }: { onClick?: () => void }) {
  const { t } = useTranslation("app");
  const { status, attachImg } = useStreamPreviewFrame();

  return (
    <Button
      type="button"
      variant="ghost"
      onClick={onClick}
      disabled={!onClick}
      aria-label={t("stream.change_source_hint")}
      className="relative aspect-video w-full shrink-0 overflow-hidden rounded-sm border border-border bg-black p-0"
    >
      <img
        ref={attachImg}
        alt=""
        className={`w-full h-full object-contain ${status === PreviewState.Live ? "" : "hidden"}`}
      />
      {status !== PreviewState.Live && (
        <div className="absolute inset-0 flex items-center justify-center">
          <span
            className={`text-sm font-mono tracking-wide ${status === PreviewState.Error ? "text-red" : "text-accent"}`}
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
