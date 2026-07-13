import { useState } from "react";
import { createPortal } from "react-dom";
import { Loader2, X, Monitor, AppWindow } from "lucide-react";
import { useTranslation } from "react-i18next";
import {
  useMonitors,
  useWindows,
  useCaptureSource,
  useSetCaptureSource,
} from "../hooks/useStreamSettings";
import { captureMonitorThumb, captureWindowThumb } from "../lib/commands";
import { CaptureSourceKind, type CaptureSource } from "../types";
import { Thumb } from "./ui/Thumb";
import { SourceCard } from "./ui/SourceCard";
import { Button } from "./ui/button";

export default function SourcePicker({ onClose }: { onClose: () => void }) {
  const { t } = useTranslation("app");
  const [tab, setTab] = useState<CaptureSourceKind>(CaptureSourceKind.Window);
  const { data: monitors = [] } = useMonitors();
  const { data: windows = [], isLoading: windowsLoading } = useWindows();
  const { data: captureSource } = useCaptureSource();
  const { mutate: saveCaptureSource } = useSetCaptureSource();

  const select = (source: CaptureSource) => {
    saveCaptureSource(source);
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
          <Monitor size={14} className="text-dim" />
          {t("stream.source_picker_title")}
        </span>
        <Button
          variant="ghost"
          size="icon"
          onClick={onClose}
          aria-label={t("stream.source_picker_close")}
        >
          <X size={16} />
        </Button>
      </div>

      {/* Tabs */}
      <div className="flex gap-2 px-4 pt-3">
        <Button
          variant={tab === CaptureSourceKind.Window ? "destructive" : "outline"}
          onClick={() => setTab(CaptureSourceKind.Window)}
          className="flex-1 py-2"
        >
          <AppWindow size={13} />
          {t("stream.tab_windows")}
        </Button>
        <Button
          variant={tab === CaptureSourceKind.Monitor ? "destructive" : "outline"}
          onClick={() => setTab(CaptureSourceKind.Monitor)}
          className="flex-1 py-2"
        >
          <Monitor size={13} />
          {t("stream.tab_fullscreen")}
        </Button>
      </div>

      {/* Grid */}
      <div className="flex-1 overflow-y-auto px-4 py-3">
        {tab === CaptureSourceKind.Monitor ? (
          <div className="grid grid-cols-2 gap-2">
            {monitors.map((m) => (
              <SourceCard
                key={m.index}
                selected={
                  captureSource?.kind === CaptureSourceKind.Monitor &&
                  captureSource.index === m.index
                }
                label={
                  t("stream.monitor_option", {
                    index: m.index + 1,
                    width: m.width,
                    height: m.height,
                  }) + (m.primary ? t("stream.monitor_primary") : "")
                }
                onSelect={() =>
                  select({ kind: CaptureSourceKind.Monitor, index: m.index })
                }
              >
                <Thumb
                  queryKey={["monitorThumb", m.index]}
                  fetcher={() => captureMonitorThumb(m.index)}
                />
              </SourceCard>
            ))}
          </div>
        ) : windowsLoading ? (
          <div className="h-full flex items-center justify-center">
            <Loader2 size={18} className="animate-spin text-dim" />
          </div>
        ) : windows.length === 0 ? (
          <p className="text-2xs text-dim font-mono tracking-wide text-center py-6">
            {t("stream.no_windows")}
          </p>
        ) : (
          <div className="grid grid-cols-2 gap-2">
            {windows.map((w) => (
              <SourceCard
                key={w.hwnd}
                selected={
                  captureSource?.kind === CaptureSourceKind.Window &&
                  captureSource.hwnd === w.hwnd
                }
                label={w.title}
                sub={w.process_name}
                onSelect={() =>
                  select({
                    kind: CaptureSourceKind.Window,
                    hwnd: w.hwnd,
                    title: w.title,
                  })
                }
              >
                <Thumb
                  queryKey={["windowThumb", w.hwnd]}
                  fetcher={() => captureWindowThumb(w.hwnd)}
                />
              </SourceCard>
            ))}
          </div>
        )}
      </div>
    </div>,
    document.body,
  );
}
