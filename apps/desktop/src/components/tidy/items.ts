// 整理建议的条目：同一个文件的一串操作合成一条，按目标位置或整理原因分组。
import { baseName, type OpKind, type OpView } from "../../api";
import { t } from "../../i18n";

export type Item = {
  group: number;
  origin: string;
  name: string;
  originDir: string;
  category: string | null;
  reason: string;
  kind: OpKind;
  dir: string | null;
  dirDisplay: string | null;
  renamedTo: string | null;
  note: string | null;
  /** 这次手动改了位置（“这次改到其他位置…”） */
  manual: boolean;
};

export type Section = { key: string; title: string; sub: string | null; items: Item[]; trash?: boolean };

/** 原文件不动、只生成新东西的动作，在分组标题上注明。 */
export const SECTION_PREFIX: Partial<Record<OpKind, string>> = {
  copy: t("复制到 "),
  extract: t("解压到 "),
  compress: t("压缩到 "),
  shortcut: t("快捷方式放到 "),
};

export const ATTENTION = new Set(["conflict", "invalid", "in_use"]);

export function toItems(ops: OpView[]): Item[] {
  const byGroup = new Map<number, OpView[]>();
  for (const o of ops) byGroup.set(o.group, [...(byGroup.get(o.group) ?? []), o]);
  return [...byGroup.values()].map((list) => {
    const last = list[list.length - 1];
    const rename = list.find((o) => o.kind === "rename");
    const origin = list[0].origin;
    return {
      group: last.group,
      origin,
      name: last.origin_name,
      originDir: list[0].origin_display.slice(0, -(last.origin_name.length + 1)),
      category: last.category,
      reason: last.reason,
      kind: last.kind,
      dir: last.dst_dir,
      dirDisplay: last.dst_dir_display,
      renamedTo: rename?.dst ? baseName(rename.dst) : null,
      note: list.find((o) => o.note)?.note ?? null,
      manual: last.rule === "手动指定",
    };
  });
}
