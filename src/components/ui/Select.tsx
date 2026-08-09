import type { ComponentProps } from "react";
import { cn } from "../../lib/utils";

export function Select({ className, ...props }: ComponentProps<"select">) {
  return (
    <select
      {...props}
      className={cn(
        "bg-bg2 border border-border rounded-sm px-2 py-2 text-xs text-text font-mono",
        className,
      )}
    />
  );
}
