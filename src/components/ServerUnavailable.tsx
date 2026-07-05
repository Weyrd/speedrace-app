import { useState } from "react";
import { useTranslation } from "react-i18next";
import { RefreshCw } from "lucide-react";
import { useAppState, useActions, Phase } from "../store";
import { Button } from "./ui/button";

export default function ServerUnavailable() {
  const state = useAppState();
  const { t } = useTranslation("app");
  const { retryConnection } = useActions();
  const [retrying, setRetrying] = useState(false);

  if (state.phase !== Phase.ServerUnavailable) return null;

  async function handleRetry() {
    setRetrying(true);
    await retryConnection();
    setRetrying(false);
  }

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 py-10 text-center">
      <span className="text-5xl text-dim">🛠️</span>
      <p className="text-2xl text-text font-mono tracking-wide font-bold">
        {t("server_unavailable.title")}
      </p>
      <p className="text-xs text-dim font-mono tracking-wide leading-relaxed whitespace-pre-line">
        {t("server_unavailable.description")}
      </p>
      <Button
        variant="start"
        onClick={handleRetry}
        disabled={retrying}
        className="mt-2 text-sm"
      >
        <RefreshCw size={15} className={retrying ? "animate-spin" : undefined} />
        {t("server_unavailable.retry")}
      </Button>
    </div>
  );
}
