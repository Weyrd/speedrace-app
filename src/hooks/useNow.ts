import { useSyncExternalStore } from "react";

let cachedNow = Date.now();
let timer: ReturnType<typeof setInterval> | undefined;
const listeners = new Set<() => void>();

function subscribe(cb: () => void) {
  listeners.add(cb);
  if (listeners.size === 1) {
    cachedNow = Date.now();
    timer = setInterval(() => {
      cachedNow = Date.now();
      listeners.forEach((fn) => fn());
    }, 1000);
  }
  return () => {
    listeners.delete(cb);
    if (listeners.size === 0) clearInterval(timer);
  };
}

const subscribeNever = () => () => {};
const getNow = () => cachedNow;

export function useNow(enabled = true): number {
  return useSyncExternalStore(enabled ? subscribe : subscribeNever, getNow);
}
