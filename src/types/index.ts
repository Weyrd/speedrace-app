// Auth
export interface User {
  username: string;
}
export const AuthState = {
  Authenticated: "authenticated",
  Unauthenticated: "unauthenticated",
} as const;
export type AuthState = (typeof AuthState)[keyof typeof AuthState];
export type AuthStatePayload =
  | { state: typeof AuthState.Authenticated; user: User }
  | { state: typeof AuthState.Unauthenticated };

// WS / connection
export const WsStatus = {
  Connected: "connected",
  Connecting: "connecting",
  Disconnected: "disconnected",
} as const;
export type WsStatus = (typeof WsStatus)[keyof typeof WsStatus];

// Domain enums - values must match Rust serde output exactly
export const PlayerStatus = {
  Preparing: "preparing",
  InProgress: "in_progress",
  Finished: "finished",
  Forfeited: "forfeited",
} as const;
export type PlayerStatus = (typeof PlayerStatus)[keyof typeof PlayerStatus];

export const LobbyStatus = {
  Waiting: "waiting",
  InProgress: "in_progress",
} as const;
export type LobbyStatus = (typeof LobbyStatus)[keyof typeof LobbyStatus];

export const RaceType = {
  Casual: "casual",
  Ranked: "ranked",
} as const;
export type RaceType = (typeof RaceType)[keyof typeof RaceType];

export const LobbyClosedReason = {
  Left: "Left",
  Deleted: "Deleted",
  DeletedByReferee: "DeletedByReferee",
  Kicked: "Kicked",
  DeletedByAdmin: "DeletedByAdmin",
} as const;
export type LobbyClosedReason =
  (typeof LobbyClosedReason)[keyof typeof LobbyClosedReason];

export interface AutosplitState {
  wasm: boolean;
  livesplit: boolean;
  splits_match?: boolean | null;
  run_in_progress?: boolean;
}

// Tauri event payloads
export interface LobbySetup {
  lobby_id: string;
  lobby_status: LobbyStatus;
  race_type: RaceType;
  code: string;
  player_status: PlayerStatus;
  stream_key: string;
  whip_url: string;
  whep_url: string;
  game_name: string;
  category_name: string[];
  max_duration_minutes: number;
  race_start_at: number | null;
  expires_at: number;
  game_id: string;
  category_id: string;
  category_split_id: string | null;
  split_resource_updated_at: string | null;
  autosplitter_updated_at: string | null;
}
export interface PlayerResult {
  player_status: PlayerStatus;
  finishing_time_ms: number | null;
  finish_position: number | null;
}
export interface SplitFiredPayload {
  index: number;
  segment_ms: number;
  new_start_ms: number;
}
export interface LobbyClosedPayload {
  lobby_id: string;
  reason: LobbyClosedReason;
}
export interface LobbyStartPayload {
  race_start_at: number;
  expires_at: number;
}

// ffmpeg stream
export const StreamStatus = {
  Idle: "idle",
  Connecting: "connecting",
  Live: "live",
  Reconnecting: "reconnecting",
  Error: "error",
} as const;
export type StreamStatus = (typeof StreamStatus)[keyof typeof StreamStatus];

export const StreamEventState = {
  ...StreamStatus,
  Stopped: "stopped",
} as const;
export type StreamEventState =
  (typeof StreamEventState)[keyof typeof StreamEventState];

// "stream:status" event payload
export interface StreamStatusPayload {
  state: StreamEventState;
  message?: string;
}

// "stream:preview" base64 JPEG frame
export interface StreamPreviewPayload {
  frame?: string;
  error?: string;
}

// "upload:status" ranked VOD upload
export const UploadPhase = {
  Preparing: "preparing",
  Uploading: "uploading",
  Processing: "processing",
  Done: "done",
  Failed: "failed",
  QuotaExhausted: "quota_exhausted",
  Abandoned: "abandoned",
} as const;
export type UploadPhase = (typeof UploadPhase)[keyof typeof UploadPhase];

export interface UploadStatusPayload {
  state: UploadPhase;
  uploaded_bytes: number;
  total_bytes: number;
  message?: string;
}

export const PreviewState = {
  Starting: "starting",
  Live: "live",
  Error: "error",
} as const;
export type PreviewState = (typeof PreviewState)[keyof typeof PreviewState];

export interface MonitorInfo {
  index: number;
  width: number;
  height: number;
  primary: boolean;
  device_name: string;
}

export interface WindowInfo {
  hwnd: number;
  title: string;
  process_name: string;
}

export const CaptureSourceKind = {
  Monitor: "monitor",
  Window: "window",
} as const;
export type CaptureSourceKind =
  (typeof CaptureSourceKind)[keyof typeof CaptureSourceKind];

export type CaptureSource =
  | { kind: typeof CaptureSourceKind.Monitor; index: number }
  | { kind: typeof CaptureSourceKind.Window; hwnd: number; title: string };

export const EncoderPref = {
  Auto: "auto",
  Nvenc: "h264_nvenc",
  Amf: "h264_amf",
  X264: "libx264",
} as const;
export type EncoderPref = (typeof EncoderPref)[keyof typeof EncoderPref];

export const ENCODER_CHOICES = [
  EncoderPref.Auto,
  EncoderPref.Nvenc,
  EncoderPref.Amf,
  EncoderPref.X264,
] as const;

export interface StreamSettings {
  bitrate_kbps: number;
  framerate: number;
  resolution: number;
  encoder: EncoderPref;
  replay_dir: string;
  replay_autodelete: boolean;
  replay_casual: boolean;
  replay_delete_uploaded: boolean;
}
