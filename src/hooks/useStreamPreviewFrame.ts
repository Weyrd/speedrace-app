import { useCallback, useRef, useState } from "react";
import { onStreamPreview } from "../lib/listeners";
import { PreviewState } from "../types";

export function useStreamPreviewFrame() {
  const [status, setStatus] = useState<PreviewState>(PreviewState.Starting);
  const statusRef = useRef(status);
  statusRef.current = status;
  const unlistenRef = useRef<(() => void) | null>(null);

  const attachImg = useCallback((node: HTMLImageElement | null) => {
    unlistenRef.current?.();
    unlistenRef.current = null;
    if (!node) return;
    unlistenRef.current = onStreamPreview((p) => {
      if (p.frame) {
        node.src = `data:image/jpeg;base64,${p.frame}`;
        if (statusRef.current !== PreviewState.Live)
          setStatus(PreviewState.Live);
      } else if (p.error) {
        if (statusRef.current !== PreviewState.Error)
          setStatus(PreviewState.Error);
      }
    });
  }, []);

  return { status, attachImg };
}
