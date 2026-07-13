import { Button } from "./button";

export function SourceCard({
  selected,
  label,
  sub,
  onSelect,
  children,
}: {
  selected: boolean;
  label: string;
  sub?: string;
  onSelect: () => void;
  children: React.ReactNode;
}) {
  return (
    <Button
      variant="outline"
      onClick={onSelect}
      className={`flex w-full flex-col items-stretch gap-1 p-1.5 text-left whitespace-normal ${
        selected ? "border-orange hover:border-orange" : ""
      }`}
    >
      <div className="bg-black rounded-sm aspect-video w-full overflow-hidden">
        {children}
      </div>
      <span className="text-2xs tracking-wide text-text truncate">{label}</span>
      {sub && (
        <span className="text-2xs tracking-wide text-dim truncate -mt-1">
          {sub}
        </span>
      )}
    </Button>
  );
}
