import { useEffect, useState } from "react";
import { createPortal } from "react-dom";
import {
  X,
  Settings,
  Keyboard,
  RotateCcw,
  LogOut,
  Volume2,
  MonitorPlay,
  Clapperboard,
  FolderOpen,
} from "lucide-react";
import { useTranslation } from "react-i18next";
import { useActions } from "../store";
import { EncoderPref, ENCODER_CHOICES } from "../types";
import { getSoundVolume, setSoundVolume, playSound, Sound } from "../lib/sound";
import {
  useFinishHotkey,
  useSetFinishHotkey,
  useUnregisterFinishHotkey,
} from "../hooks/useFinishHotkey";
import {
  useStreamSettings,
  useSetStreamSettings,
  useDetectedEncoder,
} from "../hooks/useStreamSettings";
import {
  eventToAccelerator,
  eventToLiveAccelerator,
  formatAccelerator,
} from "../lib/hotkey";
import { Button } from "./ui/button";
import { cn } from "../lib/utils";
import { tryCatch } from "../lib/tryCatch";
import { openReplayDir, pickReplayDir } from "../lib/commands";

const DEFAULT_FINISH_HOTKEY = "CmdOrCtrl+Shift+F";

const QUALITY_PRESETS = {
  720: { bitrates: [1500, 2000, 2500], defaultBitrate: 2000 },
  1080: { bitrates: [3000, 4500, 6000], defaultBitrate: 4500 },
} as const;

const REPLAY_AUDIO_KBPS = 160;

const ENCODER_LABELS: Record<EncoderPref, string> = {
  [EncoderPref.Auto]: "Auto",
  [EncoderPref.Nvenc]: "NVIDIA (NVENC)",
  [EncoderPref.Amf]: "AMD (AMF)",
  [EncoderPref.X264]: "CPU (x264)",
};

// x264 targets the bitrate, so only bitrate moves the file size
function gbPerHour(bitrateKbps: number): string {
  const bytes = ((bitrateKbps + REPLAY_AUDIO_KBPS) * 1000 * 3600) / 8;
  return (bytes / 1024 ** 3).toFixed(1);
}

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
  const { data: streamSettings } = useStreamSettings();
  const { mutate: saveStreamSettings } = useSetStreamSettings();
  const fps = streamSettings?.framerate ?? 60;
  const bitrate = streamSettings?.bitrate_kbps ?? 2000;
  const resolution = streamSettings?.resolution === 1080 ? 1080 : 720;
  const preset = QUALITY_PRESETS[resolution];
  const encoder = streamSettings?.encoder ?? EncoderPref.Auto;
  const { data: detected } = useDetectedEncoder();
  const replayDir = streamSettings?.replay_dir ?? "";
  const replayAutodelete = streamSettings?.replay_autodelete ?? true;
  const replayCasual = streamSettings?.replay_casual ?? false;
  const replayDeleteUploaded = streamSettings?.replay_delete_uploaded ?? false;

  const safeBitrate = (preset.bitrates as readonly number[]).includes(bitrate)
    ? bitrate
    : preset.defaultBitrate;

  const handleQualityChange = (next: 720 | 1080) =>
    saveStreamSettings({
      resolution: next,
      bitrate_kbps: QUALITY_PRESETS[next].defaultBitrate,
    });

  const handlePickReplayDir = async () => {
    const dir = await pickReplayDir();
    if (dir) saveStreamSettings({ replay_dir: dir });
  };

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
        if (hotkey) applyHotkey(hotkey);
        return;
      }

      setLiveCombo(eventToLiveAccelerator(e));
      const accel = eventToAccelerator(e);
      if (accel) candidate = accel;
    };

    const onKeyUp = (e: KeyboardEvent) => {
      e.preventDefault();
      if (!candidate) return;
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
      <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-4 px-5 py-5">
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

        <div className="flex flex-col gap-2">
          <span className="flex items-center gap-2 text-xs font-mono tracking-wide text-muted">
            <MonitorPlay size={14} className="text-dim" />
            {t("stream_title")}
          </span>
          <p className="text-2xs font-mono text-dim leading-relaxed">
            {t("stream_description")}
          </p>
          <label className="flex flex-col gap-1 mt-1">
            <span className="text-2xs font-mono text-dim">
              {t("resolution_label")}
            </span>
            <select
              value={resolution}
              onChange={(e) =>
                handleQualityChange(Number(e.target.value) as 720 | 1080)
              }
              className="bg-black border border-border rounded-sm px-2 py-2 text-xs text-text font-mono"
            >
              <option value={720}>720p</option>
              <option value={1080}>1080p</option>
            </select>
          </label>
          <div className="flex items-center gap-3">
            <label className="flex-1 flex flex-col gap-1">
              <span className="text-2xs font-mono text-dim">
                {t("framerate_label")}
              </span>
              <select
                value={fps}
                onChange={(e) =>
                  saveStreamSettings({ framerate: Number(e.target.value) })
                }
                className="bg-black border border-border rounded-sm px-2 py-2 text-xs text-text font-mono"
              >
                <option value={30}>30 fps</option>
                <option value={60}>60 fps</option>
              </select>
            </label>
            <label className="flex-1 flex flex-col gap-1">
              <span className="text-2xs font-mono text-dim">
                {t("bitrate_label")}
              </span>
              <select
                value={safeBitrate}
                onChange={(e) =>
                  saveStreamSettings({ bitrate_kbps: Number(e.target.value) })
                }
                className="bg-black border border-border rounded-sm px-2 py-2 text-xs text-text font-mono"
              >
                {preset.bitrates.map((kbps) => (
                  <option key={kbps} value={kbps}>
                    {kbps} kbps
                  </option>
                ))}
              </select>
            </label>
          </div>
          <p className="text-2xs font-mono text-dim leading-relaxed">
            {t("stream_size_hint", {
              resolution,
              fps,
              size: gbPerHour(safeBitrate),
            })}
          </p>
          <p className="text-2xs font-mono text-dim leading-relaxed">
            {t("stream_quality_note")}
          </p>
          <label className="flex flex-col gap-1 mt-1">
            <span className="text-2xs font-mono text-dim">
              {t("encoder_label")}
            </span>
            <select
              value={encoder}
              onChange={(e) =>
                saveStreamSettings({ encoder: e.target.value as EncoderPref })
              }
              className="bg-black border border-border rounded-sm px-2 py-2 text-xs text-text font-mono"
            >
              {ENCODER_CHOICES.map((c) => (
                <option key={c} value={c}>
                  {c === EncoderPref.Auto ? t("encoder_auto") : ENCODER_LABELS[c]}
                </option>
              ))}
            </select>
          </label>
          <p className="text-2xs font-mono text-dim leading-relaxed">
            {detected
              ? t("encoder_detected", { encoder: ENCODER_LABELS[detected] })
              : t("encoder_detecting")}
          </p>
        </div>

        <div className="flex flex-col gap-2">
          <span className="flex items-center gap-2 text-xs font-mono tracking-wide text-muted">
            <Clapperboard size={14} className="text-dim" />
            {t("replay_title")}
          </span>
          <p className="text-2xs font-mono text-dim leading-relaxed">
            {t("replay_description")}
          </p>
          <div className="flex items-center gap-2 mt-1">
            <span
              className="flex-1 truncate rounded-sm border border-border bg-black px-2 py-2 text-2xs font-mono text-text"
              title={replayDir}
            >
              {replayDir || t("replay_folder_unset")}
            </span>
            <Button
              variant="outline"
              onClick={handlePickReplayDir}
              className="px-3 border-dim"
            >
              {t("replay_change_folder")}
            </Button>
            <Button
              variant="outline"
              onClick={() => void openReplayDir()}
              className="px-3 border-dim"
            >
              <FolderOpen size={14} />
              {t("replay_open_folder")}
            </Button>
          </div>
          <label className="flex items-center gap-2 mt-1 cursor-pointer">
            <input
              type="checkbox"
              checked={replayCasual}
              onChange={(e) =>
                saveStreamSettings({ replay_casual: e.target.checked })
              }
              className="accent-orange"
            />
            <span className="text-2xs font-mono text-dim">
              {t("replay_casual_label")}
            </span>
          </label>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={replayAutodelete}
              onChange={(e) =>
                saveStreamSettings({ replay_autodelete: e.target.checked })
              }
              className="accent-orange"
            />
            <span className="text-2xs font-mono text-dim">
              {t("replay_autodelete_label")}
            </span>
          </label>
          <label className="flex items-center gap-2 cursor-pointer">
            <input
              type="checkbox"
              checked={replayDeleteUploaded}
              onChange={(e) =>
                saveStreamSettings({ replay_delete_uploaded: e.target.checked })
              }
              className="accent-orange"
            />
            <span className="text-2xs font-mono text-dim">
              {t("replay_delete_uploaded_label")}
            </span>
          </label>
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
