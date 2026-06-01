import { createPortal } from "react-dom";
import { X, Settings } from "lucide-react";
import { useTranslation } from "react-i18next";

interface SettingsPanelProps {
  onClose: () => void;
}

export default function SettingsPanel({ onClose }: SettingsPanelProps) {
  const { t } = useTranslation("settings");

  return createPortal(
    <div
      style={{ backgroundColor: "#252320" }}
      className="fixed inset-0 z-100 flex flex-col"
    >
      {/* Header */}
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <span className="flex items-center gap-2 text-xs font-mono tracking-wide text-muted">
          <Settings size={14} className="text-dim" />
          {t("tooltip")}
        </span>
        <button
          onClick={onClose}
          className="text-dim hover:text-muted transition-colors cursor-pointer bg-transparent border-none p-0.5"
          aria-label={t("close")}
        >
          <X size={16} />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 flex flex-col items-center justify-center gap-3 px-6 text-center">
        <Settings size={32} className="text-dim opacity-40" />
        <p className="text-sm font-mono tracking-wide text-muted">
          {t("coming_soon")}
        </p>
        <p className="text-2xs font-mono text-dim leading-relaxed">
          {t("coming_soon_description")}
        </p>
      </div>
    </div>,
    document.body,
  );
}
