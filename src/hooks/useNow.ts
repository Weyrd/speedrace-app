import { useSyncExternalStore } from "react";
import { createClockStore } from "../lib/clockStore";

const clock = createClockStore((tick) => {
  const id = setInterval(tick, 1000);
  return () => clearInterval(id);
});

const subscribeNever = () => () => {};

export function useNow(enabled = true): number {
  return useSyncExternalStore(
    enabled ? clock.subscribe : subscribeNever,
    clock.getNow,
  );
}
