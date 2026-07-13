import { useQuery } from "@tanstack/react-query";
import { Loader2 } from "lucide-react";

export function Thumb({
  queryKey,
  fetcher,
}: {
  queryKey: readonly unknown[];
  fetcher: () => Promise<string>;
}) {
  const { data, isError } = useQuery({
    queryKey,
    queryFn: fetcher,
    staleTime: 30_000,
    retry: false,
  });
  if (isError) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <span className="text-2xs text-dim font-mono">—</span>
      </div>
    );
  }
  if (!data) {
    return (
      <div className="w-full h-full flex items-center justify-center">
        <Loader2 size={16} className="animate-spin text-dim" />
      </div>
    );
  }
  return (
    <img
      src={`data:image/jpeg;base64,${data}`}
      alt=""
      className="w-full h-full object-contain"
    />
  );
}
