import { useTranslation } from "react-i18next";
import { Ban } from "lucide-react";
import { useAppState, useActions, Phase } from "../store";
import { Button } from "./ui/button";

export default function Banned() {
  const state = useAppState();
  const { t } = useTranslation("app");
  const { logout } = useActions();

  if (state.phase !== Phase.Banned) return null;

  return (
    <div className="flex h-full flex-col items-center justify-center gap-3 px-6 py-10 text-center">
      <span className="flex items-center justify-center rounded-sm bg-red-dim border border-red-dim p-3">
        <Ban size={28} className="text-red" />
      </span>
      <p className="text-2xl text-text font-mono tracking-wide font-bold">
        {t("banned.title")}
      </p>
      <p className="text-xs text-dim font-mono tracking-wide leading-relaxed whitespace-pre-line">
        {t("banned.description")}
      </p>
      <Button variant="start" onClick={() => logout()} className="mt-2 text-sm">
        {t("banned.logout")}
      </Button>
    </div>
  );
}
