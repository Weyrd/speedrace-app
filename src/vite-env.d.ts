/// <reference types="vite/client" />

interface ImportMetaEnv {
  readonly WEB_BASE_URL: string;
  readonly BACKEND_URL: string;
  readonly WS_DEBUG: string;
}

interface ImportMeta {
  readonly env: ImportMetaEnv;
}
