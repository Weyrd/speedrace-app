import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";

export default function Login() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation(["common", "app"]);

  const isNotLogged =
    state.phase !== Phase.Unauthenticated && state.phase !== Phase.Connecting;
  const isAuthenticating = state.phase === Phase.Connecting;

  function handleLogin() {
    console.log("handleLogin", { isNotLogged, isAuthenticating });
    if (!isNotLogged) {
      actions.login();
    }
    if (isNotLogged) {
      actions.logout();
    }
  }

  return (
    <div className=" h-full flex flex-col items-center justify-center gap-3 px-6 py-10">
      <span className="text-5xl">🏁</span>
      <span className="text-2xl font-bold tracking-widest text-text font-mono">
        {t("common:brand")}
      </span>
      <span className="text-xs text-muted tracking-wide font-mono">
        {t("common:tagline")}
      </span>

      <div className="w-full flex flex-col gap-2 mt-2 pt-8">
        <button
          onClick={handleLogin}
          className="w-full py-3.5 font-mono tracking-wider bg-orange text-white rounded cursor-pointer border-none hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isAuthenticating
            ? t("app:login.connecting")
            : isNotLogged
              ? t("app:login.disconnect")
              : t("app:login.connect")}
        </button>
      </div>
    </div>
  );
}
