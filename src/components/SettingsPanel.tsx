import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import { X, Settings, Keyboard, RotateCcw } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  useFinishHotkey,
  useSetFinishHotkey,
  useUnregisterFinishHotkey,
} from "../hooks/useFinishHotkey";
import {
  eventToAccelerator,
  eventToLiveAccelerator,
  formatAccelerator,
} from "../lib/hotkey";

const DEFAULT_FINISH_HOTKEY = "CmdOrCtrl+Shift+F";

interface SettingsPanelProps {
  onClose: () => void;
}

export default function SettingsPanel({ onClose }: SettingsPanelProps) {
  const { t } = useTranslation("settings");
  const { data: hotkey } = useFinishHotkey();
  const { mutate: applyHotkey } = useSetFinishHotkey();
  const { mutateAsync: releaseHotkey } = useUnregisterFinishHotkey();
  const [capturing, setCapturing] = useState(false);
  const [liveCombo, setLiveCombo] = useState("");

  const startCapture = async () => {
    try {
      await releaseHotkey();
      setLiveCombo("");
      setCapturing(true);
    } catch (e) {
      console.error("[settings] unregisterFinishHotkey error", e);
    }
  };

  useEffect(() => {
    if (!capturing) return;

    let candidate: string | null = null;

    const onKeyDown = (e: KeyboardEvent) => {
      e.preventDefault();
      e.stopPropagation();

      if (e.code === "Escape") {
        setCapturing(false);
        setLiveCombo("");
        if (hotkey) applyHotkey(hotkey); // re-register the previous binding
        return;
      }

      setLiveCombo(eventToLiveAccelerator(e)); // live preview in the input
      const accel = eventToAccelerator(e);
      if (accel) candidate = accel; // valid modifier(s) + key
    };

    const onKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();
      if (!candidate) return; // no complete chord yet → keep listening
      setCapturing(false);
      applyHotkey(candidate);
    };

    window.addEventListener("keydown", onKeyDown, true);
    window.addEventListener("keyup", onKeyUp, true);
    return () => {
      window.removeEventListener("keydown", onKeyDown, true);
      window.removeEventListener("keyup", onKeyUp, true);
    };
  }, [capturing, hotkey, applyHotkey]);

  const resetDefault = () => applyHotkey(DEFAULT_FINISH_HOTKEY);

  const handleClose = () => {
    if (capturing && hotkey) applyHotkey(hotkey);
    onClose();
  };

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
          onClick={handleClose}
          className="text-dim hover:text-muted transition-colors cursor-pointer bg-transparent border-none p-0.5"
          aria-label={t("close")}
        >
          <X size={16} />
        </button>
      </div>

      {/* Content */}
      <div className="flex-1 flex flex-col gap-4 px-5 py-5">
        <div className="flex flex-col gap-2">
          <span className="flex items-center gap-2 text-xs font-mono tracking-wide text-muted">
            <Keyboard size={14} className="text-dim" />
            {t("finish_hotkey_title")}
          </span>
          <p className="text-2xs font-mono text-dim leading-relaxed">
            {t("finish_hotkey_description")}
          </p>

          <div className="flex items-center gap-2 mt-1">
            <button
              onClick={startCapture}
              className={`flex-1 h-10 flex items-center justify-center text-xs font-mono tracking-wide border rounded cursor-pointer bg-transparent transition-colors ${
                capturing
                  ? "border-green text-green ring-1 ring-green/40"
                  : "border-border text-text hover:border-dim"
              }`}
            >
              {capturing
                ? liveCombo
                  ? formatAccelerator(liveCombo)
                  : " "
                : hotkey
                  ? formatAccelerator(hotkey)
                  : "—"}
            </button>
            <button
              onClick={resetDefault}
              title={t("reset_default")}
              aria-label={t("reset_default")}
              className="h-10 w-10 flex items-center justify-center text-dim hover:text-muted transition-colors cursor-pointer bg-transparent border border-border rounded"
            >
              <RotateCcw size={14} />
            </button>
          </div>
        </div>
      </div>
    </div>,
    document.body,
  );
}
