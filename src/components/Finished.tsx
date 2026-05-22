import { useTranslation } from "react-i18next";
import Header from "../components/Header";
import TitleBar from "../components/TitleBar";
import type { User, WsStatus } from "../types";

interface Props {
  user: User | null;
  wsStatus: WsStatus;
  onNewRace: () => void;
  onLogout: () => void;
}

export default function Finished({ user, wsStatus, onNewRace, onLogout }: Props) {
  const { t } = useTranslation("app");

  return (
    <div className="relative flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <Header user={user} wsStatus={wsStatus} onSettingsClick={onLogout} />
      <div className="px-3 py-3.5 flex flex-col gap-2.5 items-center">
        <span className="text-lg font-bold text-text">{t("race.finished_title")}</span>
        <span className="text-2xs text-muted">{t("race.finished_subtitle")}</span>

        <button
          onClick={onNewRace}
          className="w-full py-2 text-2xs font-mono tracking-wide border border-accent text-accent rounded cursor-pointer bg-transparent hover:bg-accent-dim transition-colors mt-2"
        >
          {t("race.new_race")}
        </button>
      </div>
    </div>
  );
}
