import { useTranslation } from "react-i18next";
import { ExternalLink } from "lucide-react";
import { openUrl } from "@tauri-apps/plugin-opener";
import { useAppState, Phase } from "../store";

const WEB_BASE = (() => {
  try {
    return new URL(import.meta.env.WEB_WAITING_LOBBY_URL).origin;
  } catch {
    return "";
  }
})();
const CREATE_LOBBY_URL = WEB_BASE ? `${WEB_BASE}/lobby` : "";

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
      <button
        onClick={() => openUrl(CREATE_LOBBY_URL)}
        className="mt-2 flex items-center justify-center gap-2 px-4 py-2.5 font-mono text-sm tracking-wide text-orange border border-orange-dim rounded-sm cursor-pointer bg-transparent hover:border-orange hover:bg-orange-dim transition-colors"
      >
        {t("idle.create_lobby")}
        <ExternalLink size={15} />
      </button>
    </div>
  );
}
