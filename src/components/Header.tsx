import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Settings, ExternalLink, Clock, Loader2 } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useAppState, Phase } from "../store";
import { Tooltip } from "./ui/Tooltip";
import SettingsPanel from "./SettingsPanel";
import { useClockOffset } from "../hooks/useClockOffset";
import { formatOffset } from "../lib/formatTime";

const WEB_LIVE_LOBBY_URL = import.meta.env.WEB_LIVE_LOBBY_URL;
const WEB_WAITING_LOBBY_URL = import.meta.env.WEB_WAITING_LOBBY_URL;

const LOBBY_PHASES: ReadonlySet<string> = new Set([
  Phase.StreamSetup,
  Phase.WaitingForStart,
  Phase.RaceInProgress,
]);

export default function Header() {
  const state = useAppState();
  const { t: tCommon } = useTranslation("common");
  const { t: tApp } = useTranslation("app");
  const { t: tSettings } = useTranslation("settings");
  const [settingsOpen, setSettingsOpen] = useState(false);
  const { offsetMs, isSynced, isSyncing, resync } = useClockOffset();

  const isAuthenticated =
    state.phase !== Phase.Unauthenticated && state.phase !== Phase.Connecting;
  const username =
    isAuthenticated && "user" in state ? state.user.username : null;

  const hasLobby = LOBBY_PHASES.has(state.phase);
  const lobbyId = hasLobby && "lobby" in state ? state.lobby.lobby_id : null;

  const webBaseUrl =
    state.phase === Phase.RaceInProgress
      ? WEB_LIVE_LOBBY_URL
      : WEB_WAITING_LOBBY_URL;

  async function handleOpenLobby() {
    if (!lobbyId) return;
    await openUrl(`${webBaseUrl}/${lobbyId}`);
  }

  return (
    <div className="px-4 py-3 flex items-center justify-between border-b border-border">
      {/* Left: connection status */}
      <span className="flex items-center gap-1.5">
        <span
          className={`w-2 h-2 rounded-full ${isAuthenticated ? "bg-green" : "bg-red"}`}
        />
        <span className="text-xs font-mono tracking-wide text-muted">
          {isAuthenticated ? username : tCommon("not_logged")}
        </span>
      </span>

      {/* Right: action buttons */}
      <span className="flex items-center gap-1">
        <Tooltip
          content={
            isSyncing
              ? tApp("header.clock_syncing")
              : isSynced
                ? tApp("header.clock_synced", { offset: formatOffset(offsetMs) })
                : tApp("header.clock_unknown")
          }
          side="bottom"
        >
          <button
            onClick={() => resync()}
            disabled={isSyncing}
            className="flex items-center gap-1 text-dim hover:text-muted transition-colors cursor-pointer bg-transparent border-none p-0.5 disabled:opacity-60 disabled:cursor-default"
            aria-label={tApp("header.clock_syncing")}
          >
            {isSyncing ? (
              <Loader2 size={13} className="animate-spin" />
            ) : (
              <Clock size={13} />
            )}
            <span className="text-2xs font-mono tracking-wide tabular-nums">
              {isSynced ? formatOffset(offsetMs) : "—"}
            </span>
          </button>
        </Tooltip>

        {lobbyId && (
          <Tooltip content={tApp("header.open_lobby")} side="bottom">
            <button
              onClick={handleOpenLobby}
              className="text-dim hover:text-muted transition-colors cursor-pointer bg-transparent border-none p-0.5"
              aria-label={tApp("header.open_lobby")}
            >
              <ExternalLink size={15} />
            </button>
          </Tooltip>
        )}

        <Tooltip content={tSettings("tooltip")} side="bottom">
          <button
            onClick={() => setSettingsOpen(true)}
            className="text-dim hover:text-muted transition-colors cursor-pointer bg-transparent border-none p-0.5"
            aria-label={tSettings("tooltip")}
          >
            <Settings size={15} />
          </button>
        </Tooltip>
      </span>

      {/* Settings panel overlay */}
      {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}
