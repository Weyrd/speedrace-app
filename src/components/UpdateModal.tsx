import { useTranslation } from "react-i18next";
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
    <div className="fixed inset-0 bg-black/70 flex items-center justify-center z-50">
      <div className="bg-bg1 border border-border rounded mx-3 p-3.5 w-full max-w-xs">
        <p className="text-text font-mono tracking-wide font-bold mb-1.5">
          {t("update_modal.title", { version })}
        </p>
        {body && (
          <p className="text-xs text-muted font-mono tracking-wide leading-relaxed mb-4 whitespace-pre-line line-clamp-4">
            {body}
          </p>
        )}
        <div className="flex gap-2">
          <Button variant="outline" onClick={onDismiss} className="flex-1 py-2 text-2xs">
            {t("update_modal.later")}
          </Button>
          <Button variant="success" onClick={onConfirm} className="flex-1 py-2 text-2xs">
            {t("update_modal.install")}
          </Button>
        </div>
      </div>
    </div>
  );
}
