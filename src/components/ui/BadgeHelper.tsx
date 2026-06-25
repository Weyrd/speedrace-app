import { Tooltip } from "./Tooltip";
import { AutosplitStatus } from "../../types";

type LobbyHeaderProps = {
  gameName: string;
  categories: string[];
  code: string;
  live?: boolean;
  label?: string;
  autosplitStatus?: AutosplitStatus;
};

export function LobbyHeader({
  gameName,
  categories,
  code,
  live = false,
  label,
  autosplitStatus,
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
          {autosplitStatus && autosplitStatus !== AutosplitStatus.None && (
            <Tooltip
              content={autosplitStatus === AutosplitStatus.Wasm ? "WASM autosplitter" : "LiveSplit"}
              side="top"
            >
              <span className="text-2xs font-mono text-green tracking-wide">split loaded</span>
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
