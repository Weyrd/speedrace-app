import type { LucideIcon } from "lucide-react";

export function SectionHeader({
  icon: Icon,
  label,
}: {
  icon: LucideIcon;
  label: string;
}) {
  return (
    <span className="flex items-center gap-2 text-xs font-mono tracking-wide text-muted">
      <Icon size={14} className="text-dim" />
      {label}
    </span>
  );
}
