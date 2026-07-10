import { useQuery } from "@tanstack/react-query";
import { getVersion } from "@tauri-apps/api/app";

export default function Footer() {
  const { data: version } = useQuery({
    queryKey: ["app-version"],
    queryFn: getVersion,
    staleTime: Infinity,
  });

  return (
    <div className="w-full flex justify-center border-t border-border">
      <p className="text-2xs text-dim tracking-wide font-mono py-2">
        v{version}
      </p>
    </div>
  );
}
