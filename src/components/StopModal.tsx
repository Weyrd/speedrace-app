import { useTranslation } from "react-i18next";

interface Props {
  raceInProgress: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export default function StopModal({
  raceInProgress,
  onConfirm,
  onCancel,
}: Props) {
  const { t } = useTranslation(["app", "common"]);
  return (
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-40">
      <div className="bg-bg1 border border-border rounded mx-3 p-3.5 w-full max-w-xs">
        <p className=" text-text font-mono tracking-wide font-bold mb-1.5">
          {t("app:stop_modal.title")}
        </p>
        <p className="text-xs text-muted font-mono tracking-wide leading-relaxed mb-4 whitespace-pre-line">
          {raceInProgress
            ? t("app:stop_modal.message_racing")
            : t("app:stop_modal.message_idle")}
        </p>
        <div className="flex gap-2">
          <button
            onClick={onCancel}
            className="flex-1 py-2 text-2xs font-mono tracking-wide border border-border text-muted rounded cursor-pointer bg-transparent hover:border-muted transition-colors"
          >
            {t("common:cancel")}
          </button>
          <button
            onClick={onConfirm}
            className="flex-1 py-2 text-2xs font-mono tracking-wide bg-red text-white rounded cursor-pointer border-none hover:opacity-90 transition-opacity"
          >
            {t("common:stop")}
          </button>
        </div>
      </div>
    </div>
  );
}
