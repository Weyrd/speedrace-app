const base = import.meta.env.DEV
  ? "http://localhost:3000"
  : (import.meta.env.WEB_BASE_URL as string);

export const webUrls = {
  lobby: (code: string) => `${base}/${code}`,
  createLobby: `${base}/live`,
} as const;
