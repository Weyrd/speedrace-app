import type { ReactNode } from "react";
import { Tooltip as RadixTooltip } from "radix-ui";

interface TooltipProps {
  content: string;
  children: ReactNode;
  side?: "top" | "bottom";
}

export function Tooltip({ content, children, side = "bottom" }: TooltipProps) {
  return (
    <RadixTooltip.Root>
      <RadixTooltip.Trigger asChild>
        <span className="inline-flex">{children}</span>
      </RadixTooltip.Trigger>
      <RadixTooltip.Portal>
        <RadixTooltip.Content
          side={side}
          sideOffset={6}
          collisionPadding={8}
          className="z-tooltip pointer-events-none px-2 py-1 rounded-sm bg-bg3 border border-border text-2xs font-mono text-muted whitespace-nowrap"
        >
          {content}
        </RadixTooltip.Content>
      </RadixTooltip.Portal>
    </RadixTooltip.Root>
  );
}
