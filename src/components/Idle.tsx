import { useTranslation } from "react-i18next";
import { useAppState, Phase } from "../store";

export default function Idle() {
  const state = useAppState();
  const { t } = useTranslation("app");

  if (state.phase !== Phase.Idle) return null;

  return (
    <div className="flex flex-col items-center justify-center gap-3 px-6 py-10 text-center">
      <span className="text-3xl text-dim">⏳</span>
      <p className="text-xs text-text font-mono tracking-wide font-bold">
        {t("idle.title")}
      </p>
      <p className="text-2xs text-dim font-mono tracking-wide leading-relaxed whitespace-pre-line">
        {t("idle.description")}
      </p>
    </div>
  );
}