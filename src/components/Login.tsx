import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";

export default function Login() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation(["common", "app"]);

  const isAuthenticating = state.phase === Phase.Connecting;

  return (
    <div className="flex flex-col items-center justify-center gap-4 px-6 py-10">
      <span className="text-2xl">🏁</span>
      <span className="text-sm font-bold tracking-widest text-text font-mono">
        {t("common:brand")}
      </span>
      <span className="text-2xs text-muted tracking-wide font-mono">
        {t("common:tagline")}
      </span>

      <div className="w-full flex flex-col gap-2 mt-2">
        <button
          onClick={() => actions.login()}
          disabled={isAuthenticating}
          className="w-full py-2 text-2xs font-mono tracking-wider bg-orange text-white rounded cursor-pointer border-none hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isAuthenticating
            ? t("app:login.connecting")
            : t("app:login.connect")}
        </button>
        {isAuthenticating && (
          <button
            onClick={() => actions.logout()}
            className="w-full py-1.5 text-2xs font-mono tracking-wider text-dim bg-transparent border border-border rounded cursor-pointer hover:text-text hover:border-muted transition-colors"
          >
            {t("app:login.disconnect")}
          </button>
        )}
      </div>
      <p className="text-2xs text-dim tracking-wide font-mono">
        {t("common:version")}
      </p>
    </div>
  );
}