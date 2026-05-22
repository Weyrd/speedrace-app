import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";

export default function Finished() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation("app");

  if (state.phase !== Phase.Finished) return null;

  return (
    <div className="flex flex-col items-center justify-center gap-3 px-6 py-10">
      <span className="text-lg font-bold text-text">
        {t("race.finished_title")}
      </span>
      <span className="text-2xs text-muted">{t("race.finished_subtitle")}</span>

      <button
        onClick={() => actions.newRace()}
        className="w-full py-2 text-2xs font-mono tracking-wide border border-accent text-accent rounded cursor-pointer bg-transparent hover:bg-accent-dim transition-colors mt-2"
      >
        {t("race.new_race")}
      </button>
    </div>
  );
}
