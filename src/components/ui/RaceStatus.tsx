export function LivePill() {
  return (
    <span className="inline-flex items-center gap-1.5 bg-red/15 border border-red/40 rounded px-2 py-0.5 text-2xs font-mono tracking-wider text-red font-bold">
      <span className="w-1.5 h-1.5 rounded-full bg-red animate-pulse" />
      LIVE
    </span>
  );
}

export function LobbyBadge({ id }: { id: string }) {
  return (
    <span className="bg-bg2 border border-border rounded px-2 py-0.5 text-2xs font-mono tracking-wide text-muted">
      <span className="text-orange">{id}</span>
    </span>
  );
}
