import { useTranslation } from "react-i18next";
import { ExternalLink } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useAppState, Phase } from "../store";
import { webUrls } from "../lib/webUrls";
import { Button } from "./ui/button";

export default function Idle() {
  const state = useAppState();
  const { t } = useTranslation("app");

  if (state.phase !== Phase.Idle) return null;

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 py-10 text-center">
      <span className="text-5xl text-dim">⏳</span>
      <p className="text-2xl text-text font-mono tracking-wide font-bold">
        {t("idle.title")}
      </p>
      <p className="text-xs text-dim font-mono tracking-wide leading-relaxed whitespace-pre-line">
        {t("idle.description")}
      </p>
      <Button variant="start" onClick={() => openUrl(webUrls.createLobby)} className="mt-2 text-sm">
        {t("idle.create_lobby")}
        <ExternalLink size={15} />
      </Button>
    </div>
  );
}
