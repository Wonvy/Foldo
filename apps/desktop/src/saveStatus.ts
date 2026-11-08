import { useEffect, useSyncExternalStore } from "react";
import { t } from "./i18n";

type SaveStatus = { kind: "idle" | "saving" | "success" | "error"; text: string; time: number; dirty: number };
let status: SaveStatus = { kind: "idle", text: "", time: 0, dirty: 0 };
const listeners = new Set<() => void>();
const drafts = new Set<string>();
let pending = 0;
let failure: string | null = null;
function update(next: Partial<SaveStatus>) {
  status = { ...status, ...next };
  listeners.forEach((listener) => listener());
}

/** Shared by all configuration writes, including saves outside the global store. */
export async function withSaveStatus<T>(operation: () => Promise<T>): Promise<T> {
  if (pending === 0) failure = null;
  ++pending;
  update({ kind: "saving", text: t("正在保存…"), time: Date.now() });
  try {
    return await operation();
  } catch (error) {
    failure = typeof error === "string" ? error : error instanceof Error ? error.message : JSON.stringify(error);
    throw error;
  } finally {
    --pending;
    if (pending === 0) {
      update({ kind: failure ? "error" : "success", text: failure ? t("保存失败：{0}", [failure]) : t("已保存"), time: Date.now() });
    }
  }
}

export function useSaveStatus() {
  return useSyncExternalStore(
    (listener) => {
      listeners.add(listener);
      return () => {
        listeners.delete(listener);
      };
    },
    () => status,
  );
}

export function useUnsaved(key: string, dirty: boolean) {
  useEffect(() => {
    if (dirty) drafts.add(key);
    else drafts.delete(key);
    update({ dirty: drafts.size });
    return () => {
      drafts.delete(key);
      update({ dirty: drafts.size });
    };
  }, [key, dirty]);
}
