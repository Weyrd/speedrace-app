import { useTranslation } from "react-i18next";

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
          <button
            onClick={onDismiss}
            className="flex-1 py-2 text-2xs font-mono tracking-wide border border-border text-muted rounded cursor-pointer bg-transparent hover:border-muted transition-colors"
          >
            {t("update_modal.later")}
          </button>
          <button
            onClick={onConfirm}
            className="flex-1 py-2 text-2xs font-mono tracking-wide bg-green text-white rounded cursor-pointer border-none hover:opacity-90 transition-opacity"
          >
            {t("update_modal.install")}
          </button>
        </div>
      </div>
    </div>
  );
}
