import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import {
  X,
  Settings,
  Keyboard,
  RotateCcw,
  LogOut,
  Volume2,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useActions } from "../store";
import { getSoundVolume, setSoundVolume, playSound, Sound } from "../lib/sound";
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
import { Button } from "./ui/button";
import { cn } from "../lib/utils";
import { tryCatch } from "../lib/tryCatch";

const DEFAULT_FINISH_HOTKEY = "CmdOrCtrl+Shift+F";

interface SettingsPanelProps {
  onClose: () => void;
}

export default function SettingsPanel({ onClose }: SettingsPanelProps) {
  const { t } = useTranslation("settings");
  const actions = useActions();
  const { data: hotkey } = useFinishHotkey();
  const { mutate: applyHotkey } = useSetFinishHotkey();
  const { mutateAsync: releaseHotkey } = useUnregisterFinishHotkey();
  const [capturing, setCapturing] = useState(false);
  const [liveCombo, setLiveCombo] = useState("");
  const [volume, setVolume] = useState(getSoundVolume);

  const startCapture = async () => {
    const { error } = await tryCatch(releaseHotkey());
    if (error) {
      console.error("[settings] unregisterFinishHotkey error", error);
      return;
    }
    setLiveCombo("");
    setCapturing(true);
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

  useEffect(() => {
    if (capturing) return;
    const onKeyDown = (e: KeyboardEvent) => {
      if (e.code === "Escape") onClose();
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [capturing, onClose]);

  const resetDefault = () => applyHotkey(DEFAULT_FINISH_HOTKEY);

  const handleVolumeChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const v = Number(e.target.value) / 100;
    setVolume(v);
    setSoundVolume(v);
  };

  const handleClose = () => {
    if (capturing && hotkey) applyHotkey(hotkey);
    onClose();
  };

  const handleLogout = () => {
    if (capturing && hotkey) applyHotkey(hotkey);
    actions.logout();
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
        <Button
          variant="ghost"
          size="icon"
          onClick={handleClose}
          aria-label={t("close")}
        >
          <X size={16} />
        </Button>
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
            <Button
              onClick={startCapture}
              className={cn(
                "flex-1 h-10",
                capturing
                  ? "border-green text-green hover:border-green ring-1 ring-green/40"
                  : "text-text hover:border-dim",
              )}
            >
              {capturing
                ? liveCombo
                  ? formatAccelerator(liveCombo)
                  : " "
                : hotkey
                  ? formatAccelerator(hotkey)
                  : "—"}
            </Button>
            <Button
              variant="ghost"
              onClick={resetDefault}
              title={t("reset_default")}
              aria-label={t("reset_default")}
              className="h-10 w-10 border border-border"
            >
              <RotateCcw size={14} />
            </Button>
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <span className="flex items-center gap-2 text-xs font-mono tracking-wide text-muted">
            <Volume2 size={14} className="text-dim" />
            {t("sound_title")}
          </span>
          <p className="text-2xs font-mono text-dim leading-relaxed">
            {t("sound_description")}
          </p>
          <div className="flex items-center gap-3 mt-1">
            <input
              type="range"
              min={0}
              max={100}
              value={Math.round(volume * 100)}
              onChange={handleVolumeChange}
              onPointerUp={() => playSound(Sound.LobbyEnter)}
              className="flex-1 accent-orange cursor-pointer"
            />
            <span className="w-10 text-right text-xs font-mono tracking-wide tabular-nums text-text">
              {Math.round(volume * 100)}%
            </span>
          </div>
        </div>
      </div>

      {/* Footer */}
      <div className="px-5 py-4 border-t border-border">
        <Button variant="danger" onClick={handleLogout} className="w-full h-10">
          <LogOut size={14} />
          {t("logout")}
        </Button>
      </div>
    </div>,
    document.body,
  );
}
