import { useState } from "react";
import { useTranslation } from "react-i18next";
import { Settings, ExternalLink, Clock, Loader2 } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useAppState, Phase } from "../store";
import { Tooltip } from "./ui/Tooltip";
import SettingsPanel from "./SettingsPanel";
import { useClockOffset } from "../hooks/useClockOffset";
import { formatOffset } from "../lib/formatTime";
import { webUrls } from "../lib/webUrls";
import { Button } from "./ui/button";

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
  const lobbyCode = hasLobby && "lobby" in state ? state.lobby.code : null;

  async function handleOpenLobby() {
    if (!lobbyCode) return;
    const url = webUrls.lobby(lobbyCode);
    await openUrl(url);
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
                ? tApp("header.clock_synced", {
                    offset: formatOffset(offsetMs),
                  })
                : tApp("header.clock_unknown")
          }
          side="bottom"
        >
          <Button
            variant="ghost"
            size="icon"
            onClick={() => resync()}
            disabled={isSyncing}
            className="gap-1"
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
          </Button>
        </Tooltip>

        {lobbyCode && (
          <Tooltip content={tApp("header.open_lobby")} side="bottom">
            <Button
              variant="ghost"
              size="icon"
              onClick={handleOpenLobby}
              aria-label={tApp("header.open_lobby")}
            >
              <ExternalLink size={15} />
            </Button>
          </Tooltip>
        )}

        <Tooltip content={tSettings("tooltip")} side="bottom">
          <Button
            variant="ghost"
            size="icon"
            onClick={() => setSettingsOpen(true)}
            aria-label={tSettings("tooltip")}
          >
            <Settings size={15} />
          </Button>
        </Tooltip>
      </span>

      {/* Settings panel overlay */}
      {settingsOpen && <SettingsPanel onClose={() => setSettingsOpen(false)} />}
    </div>
  );
}
