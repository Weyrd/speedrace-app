
export const Sound = {
  LobbyEnter: "lobby-enter",
  LobbyClosed: "lobby-closed",
  Countdown3: "countdown-3",
  Countdown2: "countdown-2",
  Countdown1: "countdown-1",
  CountdownGo: "countdown-go",
  RaceFinish: "race-finish",
  RaceForfeit: "race-forfeit",
} as const;
export type Sound = (typeof Sound)[keyof typeof Sound];

const VOLUME_KEY = "sound-volume";
const DEFAULT_VOLUME = 0.4;

function clamp(v: number): number {
  return Math.min(1, Math.max(0, v));
}

function readStoredVolume(): number {
  const raw = localStorage.getItem(VOLUME_KEY);
  if (raw == null) return DEFAULT_VOLUME;
  const v = Number(raw);
  return Number.isFinite(v) ? clamp(v) : DEFAULT_VOLUME;
}

let volume = readStoredVolume();

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

export function getSoundVolume(): number {
  return volume;
}

export function setSoundVolume(value: number): void {
  volume = clamp(value);
  localStorage.setItem(VOLUME_KEY, String(volume));
}

export function playSound(name: Sound): void {
  const el = element(name);
  el.currentTime = 0;
  el.volume = volume;
  el.play().catch(() => {});
}
