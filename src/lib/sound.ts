
export const Sound = {
  LobbyEnter: "lobby-enter",
  Countdown3: "countdown-3",
  Countdown2: "countdown-2",
  Countdown1: "countdown-1",
  CountdownGo: "countdown-go",
  RaceFinish: "race-finish",
  RaceForfeit: "race-forfeit",
} as const;
export type Sound = (typeof Sound)[keyof typeof Sound];

const cache = new Map<Sound, HTMLAudioElement>();

function element(name: Sound): HTMLAudioElement {
  let el = cache.get(name);
  if (!el) {
    el = new Audio(`/sounds/${name}.mp3`);
    el.preload = "auto";
    cache.set(name, el);
  }
  return el;
}

export function playSound(name: Sound): void {
  const el = element(name);
  el.currentTime = 0;
  el.play().catch(() => {});
}
