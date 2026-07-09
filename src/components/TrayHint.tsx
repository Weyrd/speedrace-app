import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import { onTrayHint } from "../lib/listeners";
import { hideToTray } from "../lib/commands";
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
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-40">
      <div className="bg-bg1 border border-border rounded mx-3 p-3.5 w-full max-w-xs">
        <p className="text-text font-mono tracking-wide font-bold mb-1.5">
          {t("app:tray_hint.title")}
        </p>
        <p className="text-xs text-muted font-mono tracking-wide leading-relaxed mb-4 whitespace-pre-line">
          {t("app:tray_hint.message")}
        </p>
        <Button
          variant="outline"
          onClick={dismiss}
          className="w-full py-2 text-2xs"
        >
          {t("common:got_it")}
        </Button>
      </div>
    </div>
  );
}
