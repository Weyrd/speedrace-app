import { useTranslation, Trans } from "react-i18next";
import { TriangleAlert } from "lucide-react";
import { Button } from "./ui/button";

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
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-40 px-3">
      <div className="bg-bg1 border border-red-dim rounded-sm p-4 w-full max-w-xs">
        <div className="flex items-center gap-3 mb-3">
          <span className="flex items-center justify-center bg-red-dim border border-red-dim rounded-sm p-2 shrink-0">
            <TriangleAlert size={18} className="text-red" />
          </span>
          <p className="text-text font-mono tracking-wide font-bold">
            {raceInProgress
              ? t("app:stop_modal.title_racing")
              : t("app:stop_modal.title_idle")}
          </p>
        </div>

        <p className="text-xs text-muted font-mono tracking-wide leading-relaxed mb-4">
          {raceInProgress ? (
            <Trans
              t={t}
              i18nKey="app:stop_modal.message_racing"
              components={{ red: <span className="text-red" /> }}
            />
          ) : (
            t("app:stop_modal.message_idle")
          )}
        </p>

        <div className="flex gap-2">
          <Button variant="outline" onClick={onCancel} className="flex-1 py-3">
            {raceInProgress ? t("app:stop_modal.keep_racing") : t("common:cancel")}
          </Button>
          <Button variant="destructive" onClick={onConfirm} className="flex-1 py-3">
            {raceInProgress ? t("app:stop_modal.confirm_forfeit") : t("common:stop")}
          </Button>
        </div>
      </div>
    </div>
  );
}
