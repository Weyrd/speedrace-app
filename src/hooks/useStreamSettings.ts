import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  listMonitors,
  listWindows,
  captureSupported,
  getStreamSettings,
  setStreamSettings,
  getDetectedEncoder,
  getCaptureSource,
  setCaptureSource,
  restartPreview,
} from "../lib/commands";
import { EncoderPref } from "../types";
import type { CaptureSource, StreamSettings, EncoderStatusPayload } from "../types";

export const monitorsKey = ["monitors"] as const;
export const windowsKey = ["windows"] as const;
export const streamSettingsKey = ["streamSettings"] as const;
export const captureSourceKey = ["captureSource"] as const;
export const detectedEncoderKey = ["detectedEncoder"] as const;
export const effectiveEncoderKey = ["effectiveEncoder"] as const;
export const streamAttemptKey = ["streamAttempt"] as const;
export const captureSupportedKey = ["captureSupported"] as const;

export function useMonitors() {
  return useQuery({
    queryKey: monitorsKey,
    queryFn: listMonitors,
    retry: false,
  });
}

export function useWindows() {
  return useQuery({
    queryKey: windowsKey,
    queryFn: listWindows,
    staleTime: 0,
    retry: false,
  });
}

export function useCaptureSupported() {
  return useQuery({
    queryKey: captureSupportedKey,
    queryFn: captureSupported,
    staleTime: Infinity,
  });
}

export function useStreamSettings() {
  return useQuery({ queryKey: streamSettingsKey, queryFn: getStreamSettings });
}

export function useDetectedEncoder() {
  return useQuery({
    queryKey: detectedEncoderKey,
    queryFn: getDetectedEncoder,
    refetchInterval: (q) => (q.state.data ? false : 1000),
  });
}

export function useCaptureSource() {
  return useQuery({ queryKey: captureSourceKey, queryFn: getCaptureSource });
}

export function useEffectiveEncoder() {
  return useQuery<EncoderStatusPayload | null>({
    queryKey: effectiveEncoderKey,
    queryFn: () => null,
    enabled: false,
    initialData: null,
  });
}

export function useStreamAttempt() {
  return useQuery<string | null>({
    queryKey: streamAttemptKey,
    queryFn: () => null,
    enabled: false,
    initialData: null,
  });
}

export function useSetCaptureSource() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (source: CaptureSource) => {
      await setCaptureSource(source);
      await restartPreview().catch(() => {});
      return source;
    },
    onSuccess: (source) => queryClient.setQueryData(captureSourceKey, source),
  });
}

export function useSetStreamSettings() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (patch: Partial<StreamSettings>) => {
      const cur = queryClient.getQueryData<StreamSettings>(streamSettingsKey);
      const merged: StreamSettings = {
        bitrate_kbps: patch.bitrate_kbps ?? cur?.bitrate_kbps ?? 2000,
        framerate: patch.framerate ?? cur?.framerate ?? 60,
        resolution: patch.resolution ?? cur?.resolution ?? 720,
        encoder: patch.encoder ?? cur?.encoder ?? EncoderPref.Auto,
        replay_dir: patch.replay_dir ?? cur?.replay_dir ?? "",
        replay_autodelete:
          patch.replay_autodelete ?? cur?.replay_autodelete ?? true,
        replay_casual: patch.replay_casual ?? cur?.replay_casual ?? false,
        replay_delete_uploaded:
          patch.replay_delete_uploaded ?? cur?.replay_delete_uploaded ?? false,
      };
      await setStreamSettings(
        merged.bitrate_kbps,
        merged.framerate,
        merged.resolution,
        merged.encoder,
        merged.replay_dir,
        merged.replay_autodelete,
        merged.replay_casual,
        merged.replay_delete_uploaded,
      );
      return merged;
    },
    onSuccess: (merged) => queryClient.setQueryData(streamSettingsKey, merged),
  });
}
