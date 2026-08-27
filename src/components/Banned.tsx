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
      <Ban size={40} className="text-red" />
      <p className="text-2xl text-text font-mono tracking-wide font-bold">
        {t("banned.title")}
      </p>
      <p className="text-xs text-dim font-mono tracking-wide leading-relaxed whitespace-pre-line">
        {t("banned.description")}
      </p>
      <Button variant="default" onClick={() => logout()} className="mt-2 text-sm">
        {t("banned.logout")}
      </Button>
    </div>
  );
}
