import { useTranslation } from "react-i18next";

interface Props {
  RaceInProgress: boolean;
  onConfirm: () => void;
  onCancel: () => void;
}

export default function StopModal({ RaceInProgress, onConfirm, onCancel }: Props) {
  const { t } = useTranslation(["app", "common"]);

  return (
    // Backdrop
    <div className="absolute inset-0 bg-black/70 flex items-center justify-center z-40 rounded-md">
      <div className="bg-bg1 border border-border rounded mx-3 p-3.5">
        <div className="bg-red-dim border border-red-dim rounded p-3">
          <p className="text-2xs text-text font-mono tracking-wide font-bold mb-1.5">
            {t("app:stop_modal.title")}
          </p>
          <p className="text-2xs text-muted font-mono tracking-wide leading-relaxed mb-3 whitespace-pre-line">
            {RaceInProgress
              ? t("app:stop_modal.message_racing")
              : t("app:stop_modal.message_idle")}
          </p>
          <div className="flex gap-2">
            <button
              onClick={onCancel}
              className="flex-1 py-1.5 text-2xs font-mono tracking-wide border border-border text-muted rounded cursor-pointer bg-transparent hover:border-muted transition-colors"
            >
              {t("common:cancel")}
            </button>
            <button
              onClick={onConfirm}
              className="flex-1 py-1.5 text-2xs font-mono tracking-wide bg-red text-white rounded cursor-pointer border-none hover:opacity-90 transition-opacity"
            >
              {t("common:stop")}
            </button>
          </div>
        </div>
      </div>
    </div>
  );
}