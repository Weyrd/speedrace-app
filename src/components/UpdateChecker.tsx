import { useEffect, useState } from "react";
import type { Update } from "@tauri-apps/plugin-updater";
import { checkForUpdate, installUpdate } from "../lib/updater";
import UpdateModal from "./UpdateModal";

const CHECK_INTERVAL_MS = 30 * 60 * 1000; // 30 minutes

export function UpdateChecker() {
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);

  useEffect(() => {
    const check = async () => {
      const update = await checkForUpdate();
      if (update) setPendingUpdate(update);
    };
    check();
    const id = setInterval(check, CHECK_INTERVAL_MS);
    return () => clearInterval(id);
  }, []);

  if (!pendingUpdate) return null;

  return (
    <UpdateModal
      version={pendingUpdate.version}
      body={pendingUpdate.body ?? undefined}
      onConfirm={() => {
        setPendingUpdate(null);
        installUpdate(pendingUpdate);
      }}
      onDismiss={() => setPendingUpdate(null)}
    />
  );
}
