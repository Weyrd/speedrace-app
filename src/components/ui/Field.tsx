import type { ReactNode } from "react";
import { cn } from "../../lib/utils";

export function Field({
  label,
  className,
  children,
}: {
  label: string;
  className?: string;
  children: ReactNode;
}) {
  return (
    <label className={cn("flex flex-col gap-1", className)}>
      <span className="text-2xs font-mono text-dim">{label}</span>
      {children}
    </label>
  );
}

export function Description({ children }: { children: ReactNode }) {
  return (
    <p className="text-2xs font-mono text-dim leading-relaxed">{children}</p>
  );
}
