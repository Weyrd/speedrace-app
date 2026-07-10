import { useTranslation } from "react-i18next";
import { ExternalLink } from "lucide-react";
import { useAppState, useActions, Phase } from "../store";
import { Button } from "./ui/button";

export default function Login() {
  const state = useAppState();
  const actions = useActions();
  const { t } = useTranslation(["common", "app"]);

  const isNotLogged =
    state.phase !== Phase.Unauthenticated && state.phase !== Phase.Connecting;
  const isAuthenticating = state.phase === Phase.Connecting;
  const isConnect = !isNotLogged && !isAuthenticating;

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
        <span className="text-orange">»</span> {t("common:brand")}
        <span className="text-orange">.run</span>
      </span>
      <span className="text-xs text-muted tracking-wide font-mono italic">
        {t("common:tagline")}
      </span>

      <div className="w-full flex flex-col items-center gap-2 mt-2 pt-8">
        <Button
          variant="success"
          onClick={handleLogin}
          className="w-full py-3.5 tracking-wider"
        >
          {isAuthenticating
            ? t("app:login.connecting")
            : isNotLogged
              ? t("app:login.disconnect")
              : t("app:login.connect")}
          {isConnect && <ExternalLink size={16} />}
        </Button>
        {isConnect && (
          <span className="text-2xs text-dim font-mono tracking-wide">
            {t("app:login.opens_web")}
          </span>
        )}
      </div>
    </div>
  );
}
