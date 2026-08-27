import { useState } from "react";
import { Check, Copy, X } from "lucide-react";
import { useTranslation } from "react-i18next";
import { collectDebugReport } from "../../lib/commands";
import { tryCatch } from "../../lib/tryCatch";
import { cn } from "../../lib/utils";
import { Button } from "./button";
import { Tooltip } from "./Tooltip";

type CopyState = "idle" | "copied" | "error";

export function CopyLogsButton({ className }: { className?: string }) {
  const { t } = useTranslation("app");
  const [state, setState] = useState<CopyState>("idle");

  const handleClick = async (e: React.MouseEvent) => {
    e.stopPropagation();
    const { data: report, error } = await tryCatch(collectDebugReport());
    if (error || report === null) {
      console.error("[debug] collect_debug_report error", error);
      setState("error");
      setTimeout(() => setState("idle"), 1500);
      return;
    }
    const { error: clipboardError } = await tryCatch(
      navigator.clipboard.writeText(report),
    );
    setState(clipboardError ? "error" : "copied");
    setTimeout(() => setState("idle"), 1500);
  };

  const Icon = state === "copied" ? Check : state === "error" ? X : Copy;

  return (
    <Tooltip content={t("stream.copy_logs")}>
      <Button
        type="button"
        variant="ghost"
        size="icon"
        onClick={handleClick}
        aria-label={t("stream.copy_logs")}
        className={cn(
          "bg-black/70 text-dim hover:text-text hover:bg-black/90",
          className,
        )}
      >
        <Icon size={14} />
      </Button>
    </Tooltip>
  );
}
