import { useRef, useLayoutEffect } from "react";
import { useQuery } from "@tanstack/react-query";
import { invoke } from "@tauri-apps/api/core";
import { formatTime } from "../../lib/formatTime";

interface SplitListProps {
  currentIndex?: number;
  completedTimes?: number[];
  raceElapsedMs?: number;
  currentSegmentStartMs?: number;
}

export function SplitList({
  currentIndex,
  completedTimes,
  raceElapsedMs,
  currentSegmentStartMs,
}: SplitListProps) {
  const { data: segments = [] } = useQuery({
    queryKey: ["split-segments"],
    queryFn: () => invoke<string[]>("get_split_segments"),
    staleTime: Infinity,
  });

  const activeItemRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    activeItemRef.current?.scrollIntoView({ behavior: "smooth", block: "nearest" });
  }, [currentIndex]);

  if (segments.length === 0) return null;

  const isRacing = currentIndex !== undefined;

  return (
    <div className="flex flex-col overflow-y-auto max-h-[140px] border border-border rounded-sm">
      {segments.map((name, i) => {
        const isActive = isRacing && i === currentIndex;
        const isCompleted = isRacing && i < currentIndex!;

        let timeDisplay: string | null = null;
        if (isCompleted && completedTimes?.[i] !== undefined) {
          timeDisplay = formatTime(completedTimes[i]);
        } else if (
          isActive &&
          raceElapsedMs !== undefined &&
          currentSegmentStartMs !== undefined
        ) {
          timeDisplay = formatTime(Math.max(0, raceElapsedMs - currentSegmentStartMs));
        }

        return (
          <div
            key={i}
            ref={isActive ? activeItemRef : undefined}
            className={`flex items-center justify-between px-3 py-2 border-b border-border last:border-b-0 ${isActive ? "bg-bg2" : ""}`}
          >
            <span
              className={`text-xs font-mono tracking-wide truncate ${
                isActive ? "text-orange" : isCompleted ? "text-text" : "text-dim"
              }`}
            >
              {isActive ? "> " : "  "}
              {name}
            </span>
            {timeDisplay !== null && (
              <span className="text-xs font-mono text-text tracking-wide ml-3 shrink-0">
                {timeDisplay}
              </span>
            )}
          </div>
        );
      })}
    </div>
  );
}
