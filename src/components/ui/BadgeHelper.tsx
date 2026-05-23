export function LivePill() {
  return (
    <span className="inline-flex items-center gap-1.5 bg-red/15 border border-red/40 rounded px-2 py-0.5 text-2xs font-mono tracking-wider text-red font-bold">
      <span className="w-1.5 h-1.5 rounded-full bg-red animate-pulse" />
      LIVE
    </span>
  );
}

type LobbyBadgeProps = {
  gameName: string;
  categories: string[];
};

export function LobbyBadge({ gameName, categories }: LobbyBadgeProps) {
  const lastIndex = categories.length - 1;

  return (
    <span className="bg-bg2 border border-border rounded px-2 py-0.5 text-2xs font-mono tracking-wide">
      <span className="text-orange">{gameName}</span>

      {categories.map((category, index) => {
        const isLast = index === lastIndex;

        return (
          <span key={`${category}-${index}`}>
            <span className="text-dim"> {" > "} </span>

            <span className={isLast ? "text-[#C8A84E]" : "text-dim"}>
              {category}
            </span>
          </span>
        );
      })}
    </span>
  );
}
