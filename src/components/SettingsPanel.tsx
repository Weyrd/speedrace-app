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
import { SectionHeader } from "./ui/SectionHeader";
import { Field, Description } from "./ui/Field";
import { Select } from "./ui/Select";
import { Checkbox } from "./ui/Checkbox";
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
    <div className="fixed inset-0 z-panel flex flex-col bg-bg0">
      <div className="flex items-center justify-between px-4 py-3 border-b border-border">
        <SectionHeader icon={Settings} label={t("tooltip")} />
        <Button
          variant="ghost"
          size="icon"
          onClick={handleClose}
          aria-label={t("close")}
        >
          <X size={16} />
        </Button>
      </div>

      <div className="flex-1 min-h-0 overflow-y-auto flex flex-col gap-4 px-5 py-5">
        <div className="flex flex-col gap-2">
          <SectionHeader icon={Keyboard} label={t("finish_hotkey_title")} />
          <Description>{t("finish_hotkey_description")}</Description>

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
          <SectionHeader icon={Volume2} label={t("sound_title")} />
          <Description>{t("sound_description")}</Description>
          <div className="flex items-center gap-3 mt-1">
            <input
              type="range"
              min={0}
              max={100}
              value={Math.round(volume * 100)}
              onChange={handleVolumeChange}
              onPointerUp={() => playSound(Sound.LobbyEnter)}
              className="flex-1 accent-accent cursor-pointer"
            />
            <span className="w-10 text-right text-xs font-mono tracking-wide tabular-nums text-text">
              {Math.round(volume * 100)}%
            </span>
          </div>
        </div>

        <div className="flex flex-col gap-2">
          <SectionHeader icon={MonitorPlay} label={t("stream_title")} />
          <Description>{t("stream_description")}</Description>
          <Field label={t("resolution_label")} className="mt-1">
            <Select
              value={resolution}
              onChange={(e) =>
                handleQualityChange(Number(e.target.value) as 720 | 1080)
              }
            >
              <option value={720}>720p</option>
              <option value={1080}>1080p</option>
            </Select>
          </Field>
          <div className="flex items-center gap-3">
            <Field label={t("framerate_label")} className="flex-1">
              <Select
                value={fps}
                onChange={(e) =>
                  saveStreamSettings({ framerate: Number(e.target.value) })
                }
              >
                <option value={30}>30 fps</option>
                <option value={60}>60 fps</option>
              </Select>
            </Field>
            <Field label={t("bitrate_label")} className="flex-1">
              <Select
                value={safeBitrate}
                onChange={(e) =>
                  saveStreamSettings({ bitrate_kbps: Number(e.target.value) })
                }
              >
                {preset.bitrates.map((kbps) => (
                  <option key={kbps} value={kbps}>
                    {kbps} kbps
                  </option>
                ))}
              </Select>
            </Field>
          </div>
          <Description>
            {t("stream_size_hint", {
              resolution,
              fps,
              size: gbPerHour(safeBitrate),
            })}
          </Description>
          <Description>{t("stream_quality_note")}</Description>
          <Field label={t("encoder_label")} className="mt-1">
            <Select
              value={encoder}
              onChange={(e) =>
                saveStreamSettings({ encoder: e.target.value as EncoderPref })
              }
            >
              {ENCODER_CHOICES.map((c) => (
                <option key={c} value={c}>
                  {c === EncoderPref.Auto ? t("encoder_auto") : ENCODER_LABELS[c]}
                </option>
              ))}
            </Select>
          </Field>
          <Description>
            {detected
              ? t("encoder_detected", { encoder: ENCODER_LABELS[detected] })
              : t("encoder_detecting")}
          </Description>
        </div>

        <div className="flex flex-col gap-2">
          <SectionHeader icon={Clapperboard} label={t("replay_title")} />
          <Description>{t("replay_description")}</Description>
          <div className="flex items-center gap-2 mt-1">
            <span
              className="flex-1 truncate rounded-sm border border-border bg-bg2 px-2 py-2 text-2xs font-mono text-text"
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
          <Checkbox
            checked={replayCasual}
            onChange={(checked) =>
              saveStreamSettings({ replay_casual: checked })
            }
            label={t("replay_casual_label")}
            className="mt-1"
          />
          <Checkbox
            checked={replayAutodelete}
            onChange={(checked) =>
              saveStreamSettings({ replay_autodelete: checked })
            }
            label={t("replay_autodelete_label")}
          />
          <Checkbox
            checked={replayDeleteUploaded}
            onChange={(checked) =>
              saveStreamSettings({ replay_delete_uploaded: checked })
            }
            label={t("replay_delete_uploaded_label")}
          />
        </div>
      </div>

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
