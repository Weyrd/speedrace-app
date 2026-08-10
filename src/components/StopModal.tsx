import { useState } from "react";
import { useTranslation, Trans } from "react-i18next";
import { TriangleAlert, Loader2 } from "lucide-react";
import { Modal } from "./ui/Modal";
import { Button } from "./ui/button";

interface Props {
  raceInProgress: boolean;
  onConfirm: () => void | Promise<void>;
  onCancel: () => void;
}

export default function StopModal({
  raceInProgress,
  onConfirm,
  onCancel,
}: Props) {
  const { t } = useTranslation(["app", "common"]);
  const [busy, setBusy] = useState(false);

  const handleConfirm = async () => {
    setBusy(true);
    try {
      await onConfirm();
    } catch {
      setBusy(false);
    }
  };

  return (
    <Modal
      open
      onOpenChange={(open) => !open && onCancel()}
      className="border-red-dim p-4"
    >
      <div className="flex items-center gap-3 mb-3">
        <span className="flex items-center justify-center bg-red-dim border border-red-dim rounded-sm p-2 shrink-0">
          <TriangleAlert size={18} className="text-red" />
        </span>
        <Modal.Title asChild>
          <p className="text-text font-mono tracking-wide font-bold">
            {raceInProgress
              ? t("app:stop_modal.title_racing")
              : t("app:stop_modal.title_idle")}
          </p>
        </Modal.Title>
      </div>

      <Modal.Description asChild>
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
      </Modal.Description>

      <div className="flex gap-2">
        <Button
          variant="outline"
          onClick={onCancel}
          disabled={busy}
          className="flex-1 py-3"
        >
          {raceInProgress
            ? t("app:stop_modal.keep_racing")
            : t("common:cancel")}
        </Button>
        <Button
          variant="destructive"
          onClick={handleConfirm}
          disabled={busy}
          className="flex-1 py-3"
        >
          {busy && <Loader2 size={14} className="animate-spin" />}
          {raceInProgress
            ? t("app:stop_modal.confirm_forfeit")
            : t("common:stop")}
        </Button>
      </div>
    </Modal>
  );
}
