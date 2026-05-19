import TitleBar from "./TitleBar";

interface Props {
  onLogin: () => void;
  isConnecting: boolean;
}

export default function Login({ onLogin, isConnecting }: Props) {
  return (
    <div className="flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <div className="flex flex-col items-center gap-2.5 px-3 py-6">
        <span className="text-2xl">🏁</span>
        <span className="text-sm font-bold tracking-widest text-text font-mono">MOMENTUM</span>
        <span className="text-2xs text-muted tracking-wide font-mono">speedrun racing</span>
      </div>
      <div className="h-px bg-border" />
      <div className="px-3 py-3.5">
        <button
          onClick={onLogin}
          disabled={isConnecting}
          className="w-full py-2 text-2xs font-mono tracking-wider bg-orange text-white rounded cursor-pointer border-none hover:opacity-90 transition-opacity disabled:opacity-50 disabled:cursor-not-allowed"
        >
          {isConnecting ? "Connexion..." : "Se connecter au web"}
        </button>
      </div>
      <p className="text-2xs text-dim tracking-wide text-center pb-2.5 font-mono">v1.0.0</p>
    </div>
  );
}