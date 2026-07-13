import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  listMonitors,
  listWindows,
  getStreamSettings,
  setStreamSettings,
  getCaptureSource,
  setCaptureSource,
  restartPreview,
} from "../lib/commands";
import type { CaptureSource, StreamSettings } from "../types";

export const monitorsKey = ["monitors"] as const;
export const windowsKey = ["windows"] as const;
export const streamSettingsKey = ["streamSettings"] as const;
export const captureSourceKey = ["captureSource"] as const;

export function useMonitors() {
  return useQuery({ queryKey: monitorsKey, queryFn: listMonitors });
}

export function useWindows() {
  return useQuery({ queryKey: windowsKey, queryFn: listWindows, staleTime: 0 });
}

export function useStreamSettings() {
  return useQuery({ queryKey: streamSettingsKey, queryFn: getStreamSettings });
}

export function useCaptureSource() {
  return useQuery({ queryKey: captureSourceKey, queryFn: getCaptureSource });
}

export function useSetCaptureSource() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: async (source: CaptureSource) => {
      await setCaptureSource(source);
      // reflect the new source immediately in the local preview (if running)
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
        replay_dir: patch.replay_dir ?? cur?.replay_dir ?? "",
        replay_autodelete:
          patch.replay_autodelete ?? cur?.replay_autodelete ?? true,
        replay_casual: patch.replay_casual ?? cur?.replay_casual ?? false,
      };
      await setStreamSettings(
        merged.bitrate_kbps,
        merged.framerate,
        merged.replay_dir,
        merged.replay_autodelete,
        merged.replay_casual,
      );
      return merged;
    },
    onSuccess: (merged) => queryClient.setQueryData(streamSettingsKey, merged),
  });
}
