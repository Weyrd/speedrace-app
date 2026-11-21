import { useEffect, useState } from "react";
import { useTranslation } from "react-i18next";
import type { Update } from "@tauri-apps/plugin-updater";
import { checkForUpdate, installUpdate } from "../lib/updater";
import { useAppState, Phase } from "../store";
import UpdateModal from "./UpdateModal";

const CHECK_INTERVAL_MS = 30 * 60 * 1000; // 30 minutes

export function UpdateChecker() {
  const state = useAppState();
  const { t } = useTranslation("app");
  const [pendingUpdate, setPendingUpdate] = useState<Update | null>(null);
  const [installing, setInstalling] = useState(false);
  const [noneFound, setNoneFound] = useState(false);

  useEffect(() => {
    const check = async () => {
      const update = await checkForUpdate();
      if (update) setPendingUpdate(update);
    };
    check();
    const id = setInterval(check, CHECK_INTERVAL_MS);
    return () => clearInterval(id);
  }, []);

  if (state.phase === Phase.Outdated) {
    const installRequired = async () => {
      setInstalling(true);
      setNoneFound(false);
      const update = pendingUpdate ?? (await checkForUpdate());
      if (update) {
        await installUpdate(update);
      } else {
        setNoneFound(true);
      }
      setInstalling(false);
    };
    return (
      <UpdateModal
        required
        busy={installing}
        body={
          noneFound
            ? `${t("update_modal.required_body")}\n${t("update_modal.none_found")}`
            : t("update_modal.required_body")
        }
        onConfirm={installRequired}
      />
    );
  }

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
