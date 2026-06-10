import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  getFinishHotkey,
  setFinishHotkey,
  unregisterFinishHotkey,
} from "../lib/commands";

export const finishHotkeyKey = ["finishHotkey"] as const;

export function useFinishHotkey() {
  return useQuery({
    queryKey: finishHotkeyKey,
    queryFn: getFinishHotkey,
  });
}

export function useSetFinishHotkey() {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: setFinishHotkey,
    onSuccess: (_data, accelerator) => {
      queryClient.setQueryData(finishHotkeyKey, accelerator);
    },
  });
}

export function useUnregisterFinishHotkey() {
  return useMutation({ mutationFn: unregisterFinishHotkey });
}
