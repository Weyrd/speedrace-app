export default function TitleBar() {
  return (
    <div className="bg-bg1 px-2.5 py-1.5 flex items-center gap-1.5 border-b border-border">
      <span className="w-2 h-2 rounded-full bg-red" />
      <span className="w-2 h-2 rounded-full bg-orange" />
      <span className="w-2 h-2 rounded-full bg-green" />
      <span className="text-2xs text-dim font-mono tracking-wide ml-1.5">MOMENTUM</span>
    </div>
  );
}