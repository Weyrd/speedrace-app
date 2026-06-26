import { MonitorCheck } from "lucide-react";
import { Tooltip } from "./Tooltip";
import { AutosplitState } from "../../types";
import livesplitIcon from "../../assets/livesplit.svg";

type LobbyHeaderProps = {
  gameName: string;
  categories: string[];
  code: string;
  live?: boolean;
  label?: string;
  autosplit?: AutosplitState;
};

export function LobbyHeader({
  gameName,
  categories,
  code,
  live = false,
  label,
  autosplit,
}: LobbyHeaderProps) {
  const dash = code.indexOf("-");
  const codePrefix = dash >= 0 ? code.slice(0, dash) : code;
  const codeSuffix = dash >= 0 ? code.slice(dash) : "";

  return (
    <div className="border border-border rounded-sm bg-bg1 px-3.5 py-3 flex flex-col gap-1.5">
      <div className="flex items-center justify-between">
        {live ? (
          <span className="inline-flex items-center gap-1.5 text-2xs font-mono tracking-wider uppercase text-red font-bold">
            <span className="w-1.5 h-1.5 rounded-full bg-red animate-pulse" />
            LIVE
          </span>
        ) : (
          <span className="inline-flex items-center gap-1.5 text-2xs font-mono tracking-wider uppercase text-dim">
            <span className="text-orange">»</span>
            {label}
          </span>
        )}

        <div className="flex items-center gap-2">
          {autosplit?.wasm && (
            <Tooltip content="WASM autosplitter connected" side="top">
              <MonitorCheck className="w-3.5 h-3.5 text-green" />
            </Tooltip>
          )}
          {autosplit?.livesplit && (
            <Tooltip content="LiveSplit connected" side="top">
              <img src={livesplitIcon} alt="LiveSplit" className="w-3.5 h-3.5" />
            </Tooltip>
          )}
          <span className="bg-bg2 border border-border rounded-sm px-2 py-0.5 text-2xs font-mono tracking-wider">
            <span className="text-muted">{codePrefix}</span>
            <span className="text-orange">{codeSuffix}</span>
          </span>
        </div>
      </div>

      <span className="text-base font-bold font-mono tracking-wide text-text leading-tight">
        {gameName}
      </span>

      {categories.length > 0 && (
        <span className="text-2xs font-mono tracking-wide text-dim">
          {categories.join(" · ")}
        </span>
      )}
    </div>
  );
}
