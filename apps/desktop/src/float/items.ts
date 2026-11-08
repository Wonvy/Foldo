import { baseName, type OpKind, type PlanView } from "../api";
import { t } from "../i18n";

export type View = "none" | "category" | "extension" | "target" | "rule";
export const VIEWS: View[] = ["none", "category", "extension", "target", "rule"];
export type FloatItem = {
  group: number;
  path: string;
  name: string;
  category: string | null;
  kind: OpKind;
  dir: string | null;
  rule: string;
};
export type FloatGroup = { key: string; title: string; items: FloatItem[] };

export function pathKey(path: string): string {
  const normalized = path.replaceAll("\\", "/");
  return /^[a-z]:\//i.test(normalized) || normalized.startsWith("//") ? normalized.toLowerCase() : normalized;
}

export function mergePaths(previous: string[], incoming: string[]): string[] {
  const paths = new Map(previous.map((path) => [pathKey(path), path]));
  for (const path of incoming) if (!paths.has(pathKey(path))) paths.set(pathKey(path), path);
  return [...paths.values()];
}

export function toItems(plan: PlanView | null): FloatItem[] {
  const last = new Map<number, PlanView["ops"][number]>();
  for (const op of plan?.ops ?? []) last.set(op.group, op);
  return [...last.values()].map((op) => ({
    group: op.group,
    path: op.origin,
    name: op.origin_name,
    category: op.category,
    kind: op.kind,
    dir: op.dst_dir_display,
    rule: op.rule,
  }));
}

export function groupItems(items: FloatItem[], view: View): FloatGroup[] {
  const groups = new Map<string, FloatGroup>();
  for (const item of items) {
    let key = "all";
    let title = "";
    if (view === "category") title = t((key = item.category ?? "其他"));
    if (view === "extension") {
      const dot = item.name.lastIndexOf(".");
      key = item.category === "文件夹" ? "folder" : dot > 0 ? item.name.slice(dot).toLowerCase() : "";
      title = key === "folder" ? t("文件夹") : key || t("无扩展名");
    }
    if (view === "target") {
      key = item.kind === "trash" ? "trash" : (item.dir ?? "");
      title = key === "trash" ? t("回收站") : baseName(key) || t("原位置");
    }
    if (view === "rule") title = key = item.rule;
    key = `${view}:${key}`;
    if (!groups.has(key)) groups.set(key, { key, title, items: [] });
    groups.get(key)!.items.push(item);
  }
  return [...groups.values()];
}

export function loadView(): View {
  try {
    const view = localStorage.getItem("foldo.float.view") as View;
    if (VIEWS.includes(view)) return view;
  } catch {
    /* Storage is optional. */
  }
  return "category";
}
