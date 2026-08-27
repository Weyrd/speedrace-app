import { useTranslation } from "react-i18next";
import { useAppState, useActions, Phase } from "../store";
import { Button } from "./ui/button";

export default function Login() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation(["common", "app"]);

  const isNotLogged =
    state.phase !== Phase.Unauthenticated && state.phase !== Phase.Connecting;
  const isAuthenticating = state.phase === Phase.Connecting;

  function handleLogin() {
    if (isNotLogged) {
      actions.logout();
      return;
    }
    actions.login();
  }

  return (
    <div className="h-full flex flex-col items-center justify-center gap-3 px-6 py-10">
      <span className="text-3xl font-bold tracking-wide text-text font-mono">
        {t("common:brand")}
        <span className="text-accent">.</span>run
      </span>

      <div className="w-full flex flex-col items-center gap-2 pt-10">
        <Button
          variant="success"
          size="lg"
          onClick={handleLogin}
          className="w-full"
        >
          {isAuthenticating
            ? t("app:login.connecting")
            : isNotLogged
              ? t("app:login.disconnect")
              : t("app:login.connect")}
        </Button>
      </div>
    </div>
  );
}
