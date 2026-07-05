import { useCallback } from "react";
import {
  useQuery,
  useQueryClient,
  type QueryClient,
} from "@tanstack/react-query";
import { syncClock } from "../lib/commands";

const CLOCK_KEY = ["clock-offset"] as const;

export function ensureClockFresh(qc: QueryClient) {
  return qc.fetchQuery({
    queryKey: CLOCK_KEY,
    queryFn: () => syncClock(false),
    staleTime: 0,
  });
}

// Force a fresh measurement (bypasses the Rust cache); updates every observer.
export function resyncClock(qc: QueryClient) {
  return qc.fetchQuery({
    queryKey: CLOCK_KEY,
    queryFn: () => syncClock(true),
    staleTime: 0,
  });
}

// Offset (ms) to add to local time; Rust owns caching, resync forces a refresh.
export function useClockOffset() {
  const qc = useQueryClient();

  const query = useQuery({
    queryKey: CLOCK_KEY,
    queryFn: () => syncClock(false),
    staleTime: Infinity,
    gcTime: Infinity,
  });

  const resync = useCallback(() => resyncClock(qc), [qc]);

  return {
    offsetMs: query.data?.offset_ms ?? 0,
    syncedAt: query.data?.synced_at ?? null,
    isSynced: query.data != null,
    isSyncing: query.isFetching,
    resync,
  };
}
