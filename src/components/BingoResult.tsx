import { ExternalLink } from "lucide-react";
import { useTranslation } from "react-i18next";
import { openUrl } from "@tauri-apps/plugin-opener";
import { BingoOutcome, type BingoResult as BingoResultData } from "../types";
import { formatTime } from "../lib/formatTime";
import { webUrls } from "../lib/webUrls";
import { Button } from "./ui/button";

export default function BingoResult({ bingo }: { bingo: BingoResultData }) {
  const { t } = useTranslation("app");

  const titleColor =
    bingo.outcome === BingoOutcome.Won
      ? "text-green"
      : bingo.outcome === BingoOutcome.Lost
        ? "text-muted"
        : "text-text";

  return (
    <>
      <div className="flex flex-col items-center gap-1">
        <span
          className={`text-4xl font-bold font-mono tracking-wide ${titleColor}`}
        >
          {t(`bingo.${bingo.outcome}`)}
        </span>
        {bingo.outcome === BingoOutcome.Lost && bingo.winners.length > 0 && (
          <span className="text-2xs text-dim font-mono tracking-wide">
            {t("bingo.won_by", { names: bingo.winners.join(", ") })}
          </span>
        )}
      </div>

      <div className="flex flex-col items-center gap-1">
        <span className="text-2xl font-bold font-mono tracking-wide text-text">
          {bingo.claimed_squares}/{bingo.total_squares}
        </span>
        <span className="text-2xs text-dim font-mono tracking-wide">
          {t("bingo.squares")}
        </span>
      </div>

      <div className="flex flex-col items-center gap-1">
        <span className="text-2xl font-bold font-mono tracking-wide text-text">
          {formatTime(bingo.duration_ms)}
        </span>
        <span className="text-2xs text-dim font-mono tracking-wide">
          {t("bingo.duration")}
        </span>
      </div>

      <Button
        variant="outline"
        onClick={() => void openUrl(webUrls.history(bingo.lobby_code))}
        className="w-full py-2"
      >
        <ExternalLink size={14} />
        {t("bingo.view_results")}
      </Button>
    </>
  );
}
