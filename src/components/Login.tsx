import { useTranslation } from "react-i18next";
import TitleBar from "./TitleBar";

interface Props {
  onLogin: () => void;
  onLogout: () => void;
  isAuthenticating: boolean;
  isConnected: boolean;
}

export default function Login({ onLogin, onLogout, isAuthenticating, isConnected }: Props) {
  const { t } = useTranslation(["common", "app"]);

  return (
    <div className="flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <div className="flex flex-col items-center gap-2.5 px-3 py-6">
        <span className="text-2xl">🏁</span>
        <span className="text-sm font-bold tracking-widest text-text font-mono">{t("common:brand")}</span>
        <span className="text-2xs text-muted tracking-wide font-mono">{t("common:tagline")}</span>
      </div>
      <div className="h-px bg-border" />
      <div className="px-3 py-3.5 flex flex-col gap-2">
        <button
          onClick={onLogin}
          disabled={isAuthenticating}
          className="w-full py-2 text-2xs font-mono tracking-wider bg-orange text-white rounded cursor-pointer border-none hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isAuthenticating ? t("app:login.connecting") : t("app:login.connect")}
        </button>
        {(isAuthenticating || isConnected) && (
          <button
            onClick={onLogout}
            className="w-full py-1.5 text-2xs font-mono tracking-wider text-dim bg-transparent border border-border rounded cursor-pointer hover:text-text hover:border-muted transition-colors"
          >
            {t("app:login.disconnect", "Disconnect")}
          </button>
        )}
      </div>
      <p className="text-2xs text-dim tracking-wide text-center pb-2.5 font-mono">{t("common:version")}</p>
    </div>
  );
}