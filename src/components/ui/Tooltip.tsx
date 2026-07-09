import { useRef, useState, useEffect, type ReactNode } from "react";
import { createPortal } from "react-dom";

interface TooltipProps {
  content: string;
  children: ReactNode;
  side?: "top" | "bottom";
}

const PADDING = 8;

export function Tooltip({ content, children, side = "bottom" }: TooltipProps) {
  const [visible, setVisible] = useState(false);
  const [style, setStyle] = useState<React.CSSProperties>({});
  const triggerRef = useRef<HTMLSpanElement>(null);
  const tooltipRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    if (!visible || !triggerRef.current || !tooltipRef.current) return;
    const triggerRect = triggerRef.current.getBoundingClientRect();
    const tooltipRect = tooltipRef.current.getBoundingClientRect();
    const gap = 6;
    const vw = window.innerWidth;

    let left = triggerRect.left + triggerRect.width / 2 - tooltipRect.width / 2;
    left = Math.max(PADDING, Math.min(left, vw - tooltipRect.width - PADDING));

    if (side === "bottom") {
      setStyle({
        position: "fixed",
        top: triggerRect.bottom + gap,
        left,
        zIndex: 9999,
      });
    } else {
      setStyle({
        position: "fixed",
        top: triggerRect.top - gap - tooltipRect.height,
        left,
        zIndex: 9999,
      });
    }
  }, [visible, side]);

  return (
    <>
      <span
        ref={triggerRef}
        onMouseEnter={() => setVisible(true)}
        onMouseLeave={() => setVisible(false)}
        className="inline-flex"
      >
        {children}
      </span>
      {visible &&
        createPortal(
          <div
            ref={tooltipRef}
            style={style}
            className="pointer-events-none px-2 py-1 rounded bg-bg3 border border-border text-2xs font-mono text-muted whitespace-nowrap shadow-lg"
          >
            {content}
          </div>,
          document.body,
        )}
    </>
  );
}
