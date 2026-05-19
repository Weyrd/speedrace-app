import { useState, useRef, useCallback, useEffect } from "react";
import Header from "../components/Header";
import TitleBar from "../components/TitleBar";
import type { User, LobbySetup, WsStatus } from "../types";
import { notifyStreamReady } from "../lib/tauri";
import { WhipClient } from "../stream/whip";

interface Props {
  user: User | null;
  wsStatus: WsStatus;
  lobby: LobbySetup;
  /**
   * Called once WHIP is confirmed live and Rust has been notified.
   * `whipClient` is passed up so the parent can stop the stream later
   * (e.g. on forfeit or race end) regardless of which screen is mounted.
   */
  onStreamReady: (whipClient: WhipClient) => void;
  onLogout: () => void;
}

type Source = "screen" | "camera";

export default function StreamSetup({
  user,
  wsStatus,
  lobby,
  onStreamReady,
  onLogout,
}: Props) {
  const [source, setSource] = useState<Source>("screen");
  const [isPreviewing, setIsPreviewing] = useState(false);
  const [isPublishing, setIsPublishing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const videoRef = useRef<HTMLVideoElement>(null);
  const streamRef = useRef<MediaStream | null>(null);
  const whipRef = useRef<WhipClient | null>(null);

  // Stop preview tracks when source changes or component unmounts
  const stopPreview = useCallback(() => {
    streamRef.current?.getTracks().forEach(t => t.stop());
    streamRef.current = null;
    if (videoRef.current) videoRef.current.srcObject = null;
    setIsPreviewing(false);
  }, []);

  useEffect(() => () => stopPreview(), [stopPreview]);

  const startPreview = useCallback(async () => {
    stopPreview(); // clean up previous
    setError(null);
    try {
      const media =
        source === "screen"
          ? await navigator.mediaDevices.getDisplayMedia({
              video: { frameRate: 30 },
              audio: true,
            })
          : await navigator.mediaDevices.getUserMedia({
              video: true,
              audio: true,
            });

      streamRef.current = media;
      if (videoRef.current) videoRef.current.srcObject = media;

      // if stop
      media.getVideoTracks()[0].addEventListener("ended", () => {
        setIsPreviewing(false);
        streamRef.current = null;
      });

      setIsPreviewing(true);
    } catch (e) {
      // User cancelled permission prompt
      if (e instanceof DOMException && e.name === "NotAllowedError") return;
      setError("Impossible d'accéder à la source. Réessaie.");
    }
  }, [source, stopPreview]);

  const handlePublish = useCallback(async () => {
    if (!streamRef.current) return;
    setIsPublishing(true);
    setError(null);

    const client = new WhipClient();
    whipRef.current = client;

    try {
      await client.start(lobby.whip_url, streamRef.current);
      await notifyStreamReady(lobby.lobby_id);
      // Pass the live client up 
      onStreamReady(client);
    } catch (e) {
      console.error("[stream] WHIP publish error", e);
      client.stop();
      whipRef.current = null;
      setError(
        e instanceof Error ? e.message : "Erreur de connexion au stream."
      );
      setIsPublishing(false);
    }
  }, [lobby.whip_url, lobby.lobby_id, onStreamReady]);

  return (
    <div className="flex flex-col bg-bg0 rounded-md border border-border overflow-hidden">
      <TitleBar />
      <Header user={user} wsStatus={wsStatus} onSettingsClick={onLogout} />
      <div className="px-3 py-3.5 flex flex-col gap-2.5">

        {/* Lobby badge */}
        <div className="flex items-center justify-between">
          <span className="text-2xs text-muted font-mono tracking-wide">Lobby</span>
          <span className="bg-bg2 border border-border rounded px-2 py-0.5 text-2xs font-mono tracking-wide">
            <span className="text-orange">{lobby.lobby_id}</span>
          </span>
        </div>

        {/* Source selector */}
        <div className="flex gap-1.5">
          {(["screen", "camera"] as Source[]).map(s => (
            <button
              key={s}
              onClick={() => { setSource(s); stopPreview(); setError(null); }}
              className={`
                flex-1 py-1 text-2xs font-mono tracking-wide rounded border cursor-pointer transition-colors
                ${source === s
                  ? "bg-orange/10 border-orange text-orange"
                  : "bg-transparent border-border text-muted opacity-40 hover:opacity-60"
                }
              `}
            >
              {s === "screen" ? "Écran" : "Caméra"}
            </button>
          ))}
        </div>

        {/* Preview */}
        <div
          onClick={!isPreviewing ? startPreview : undefined}
          className="bg-black border border-border rounded h-16 flex items-center justify-center overflow-hidden relative cursor-pointer group"
        >
          <div
            className="absolute inset-0 opacity-20"
            style={{
              backgroundImage:
                "repeating-linear-gradient(0deg, rgba(255,255,255,0.08) 0px, rgba(255,255,255,0.08) 1px, transparent 1px, transparent 3px)",
            }}
          />
          {isPreviewing ? (
            <video
              ref={videoRef}
              autoPlay
              muted
              className="w-full h-full object-cover"
            />
          ) : (
            <span className="text-2xs text-dim font-mono tracking-wide z-10 group-hover:text-muted transition-colors">
              PREVIEW — cliquer pour capturer
            </span>
          )}
        </div>

        {error && (
          <p className="text-2xs text-red font-mono tracking-wide leading-relaxed">
            ⚠ {error}
          </p>
        )}

        <button
          onClick={handlePublish}
          disabled={!isPreviewing || isPublishing}
          className="w-full py-2 text-2xs font-mono tracking-wider bg-red text-white rounded border-none cursor-pointer hover:opacity-90 transition-opacity disabled:opacity-40 disabled:cursor-not-allowed"
        >
          {isPublishing ? "Connexion au stream..." : "● PUBLIER"}
        </button>
      </div>
    </div>
  );
}