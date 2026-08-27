import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { onTrayHint } from "../lib/listeners";
import { hideToTray } from "../lib/commands";
import { Modal } from "./ui/Modal";
import { Button } from "./ui/button";

export function TrayHint() {
  const { t } = useTranslation(["app", "common"]);
  const [open, setOpen] = useState(false);

  useEffect(() => onTrayHint(() => setOpen(true)), []);

  const dismiss = () => {
    setOpen(false);
    void hideToTray();
  };

  if (!open) return null;

  return (
    <Modal open onOpenChange={(next) => !next && dismiss()}>
      <Modal.Title asChild>
        <p className="text-text font-mono tracking-wide font-bold mb-1.5">
          {t("app:tray_hint.title")}
        </p>
      </Modal.Title>
      <Modal.Description asChild>
        <p className="text-xs text-muted font-mono tracking-wide leading-relaxed mb-4 whitespace-pre-line">
          {t("app:tray_hint.message")}
        </p>
      </Modal.Description>
      <Button
        variant="outline"
        onClick={dismiss}
        className="w-full py-2 text-2xs"
      >
        {t("common:got_it")}
      </Button>
    </Modal>
  );
}
