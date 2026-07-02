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

let audioCtx: AudioContext | null = null;
const bufferCache = new Map<Sound, AudioBuffer>();

function ctx(): AudioContext {
  if (!audioCtx) audioCtx = new AudioContext();
  return audioCtx;
}

async function decode(name: Sound): Promise<AudioBuffer> {
  const cached = bufferCache.get(name);
  if (cached) return cached;
  const res = await fetch(`/sounds/${name}.mp3`);
  const buf = await ctx().decodeAudioData(await res.arrayBuffer());
  bufferCache.set(name, buf);
  return buf;
}

export async function primeCountdown(sounds: readonly Sound[]): Promise<void> {
  await ctx()
    .resume()
    .catch(() => {});
  await Promise.all(sounds.map((s) => decode(s).catch(() => {})));
}

export function scheduleCountdown(
  beeps: readonly { sound: Sound; atMs: number }[],
): () => void {
  const c = ctx();
  void c.resume();
  const anchorMs = Date.now();
  const anchorCtx = c.currentTime;
  const sources: AudioBufferSourceNode[] = [];
  for (const b of beeps) {
    const buf = bufferCache.get(b.sound);
    if (!buf) continue;
    const when = anchorCtx + Math.max(0, (b.atMs - anchorMs) / 1000);
    const src = c.createBufferSource();
    src.buffer = buf;
    const gain = c.createGain();
    gain.gain.value = volume;
    src.connect(gain).connect(c.destination);
    src.start(when);
    sources.push(src);
  }
  return () => {
    for (const s of sources) {
      try {
        s.stop();
      } catch {}
    }
  };
}
