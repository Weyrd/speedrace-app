import { useTranslation } from "react-i18next";
import { Modal } from "./ui/Modal";
import { Button } from "./ui/button";

interface Props {
  version: string;
  body?: string;
  onConfirm: () => void;
  onDismiss: () => void;
}

export default function UpdateModal({
  version,
  body,
  onConfirm,
  onDismiss,
}: Props) {
  const { t } = useTranslation("app");
  return (
    <Modal open onOpenChange={(open) => !open && onDismiss()}>
      <Modal.Title asChild>
        <p className="text-text font-mono tracking-wide font-bold mb-1.5">
          {t("update_modal.title", { version })}
        </p>
      </Modal.Title>
      {body && (
        <Modal.Description asChild>
          <p className="text-xs text-muted font-mono tracking-wide leading-relaxed mb-4 whitespace-pre-line line-clamp-4">
            {body}
          </p>
        </Modal.Description>
      )}
      <div className="flex gap-2">
        <Button
          variant="outline"
          onClick={onDismiss}
          className="flex-1 py-2 text-2xs"
        >
          {t("update_modal.later")}
        </Button>
        <Button
          variant="success"
          onClick={onConfirm}
          className="flex-1 py-2 text-2xs"
        >
          {t("update_modal.install")}
        </Button>
      </div>
    </Modal>
  );
}
