export interface ClockStore {
  subscribe(cb: () => void): () => void;
  getNow(): number;
}

export function createClockStore(
  schedule: (tick: () => void) => () => void,
): ClockStore {
  let cachedNow = Date.now();
  const listeners = new Set<() => void>();
  let stopSchedule: (() => void) | undefined;

  function tick() {
    cachedNow = Date.now();
    listeners.forEach((fn) => fn());
  }

  function subscribe(cb: () => void) {
    listeners.add(cb);
    if (listeners.size === 1) {
      cachedNow = Date.now();
      stopSchedule = schedule(tick);
    }
    return () => {
      listeners.delete(cb);
      if (listeners.size === 0) stopSchedule?.();
    };
  }

  function getNow() {
    return cachedNow;
  }

  return { subscribe, getNow };
}
