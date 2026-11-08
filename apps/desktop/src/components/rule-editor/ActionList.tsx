// 规则的动作：移动、复制、改名、解压、压缩、快捷方式、回收站、保持原位，可以连续多步。
import {
  ArrowRight,
  ChevronDown,
  Copy,
  FolderOpen,
  Link2,
  Lock,
  Package,
  PackageOpen,
  PenLine,
  Plus,
  Trash,
  X,
  type LucideIcon,
} from "lucide-react";
import { t } from "../../i18n";
import { ACTION_DEFAULT_TO, tilde, type ActionKind, type ActionRow, type DestKind } from "../../rules";
import { pickFolder } from "../../store";
import { useMenu } from "../ui";
import VarMenu from "../VarMenu";

const ACTIONS: { kind: ActionKind; label: string; icon: LucideIcon }[] = [
  { kind: "move", label: t("移动"), icon: ArrowRight },
  { kind: "copy", label: t("复制"), icon: Copy },
  { kind: "rename", label: t("改名"), icon: PenLine },
  { kind: "extract", label: t("解压"), icon: PackageOpen },
  { kind: "compress", label: t("压缩"), icon: Package },
  { kind: "shortcut", label: t("快捷方式"), icon: Link2 },
  { kind: "trash", label: t("移到回收站"), icon: Trash },
  { kind: "keep", label: t("保持原位"), icon: Lock },
];

const PLACEHOLDER: Record<DestKind, string> = {
  move: t("目标文件夹，如 照片/{modified:%Y}/"),
  copy: t("目标文件夹，如 ~/备份/"),
  rename: t("新文件名，如 {exif.taken:%Y%m%d}_{name}"),
  extract: t("解压到的文件夹，相对压缩包所在位置，如 {stem}/"),
  compress: t("压缩包名称，相对文件所在位置，如 {stem}.zip"),
  shortcut: t("放快捷方式的文件夹，如 ~/Desktop/常用/"),
};

export default function ActionList({
  rows,
  folder,
  onChange,
}: {
  rows: ActionRow[];
  folder: boolean;
  onChange: (rows: ActionRow[]) => void;
}) {
  const update = (i: number, patch: Partial<ActionRow>) => onChange(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const menu = useMenu();
  const defaultTo = (k: ActionKind) => (k === "trash" || k === "keep" ? undefined : ACTION_DEFAULT_TO[k]);
  const pickKind = (el: HTMLElement, i: number) =>
    menu.open(
      el,
      ACTIONS.filter((a) => !(folder && a.kind === "extract")).map(({ kind, label, icon: Icon }) => ({
        label,
        icon: <Icon size={15} />,
        onClick: () => {
          // 原来的目标为空或是旧动作的默认值时，换成新动作的默认值
          const old = rows[i];
          const custom = old.to && old.to !== defaultTo(old.kind);
          update(i, { kind, to: custom ? old.to : (defaultTo(kind) ?? "") });
        },
      })),
    );
  return (
    <div className="cond-group">
      {rows.map((a, i) => {
        const cur = ACTIONS.find((x) => x.kind === a.kind) ?? ACTIONS[0];
        return (
          <div key={i} className="action-row">
            <button className="action-kind" onClick={(e) => pickKind(e.currentTarget, i)}>
              <cur.icon size={15} />
              <span>{cur.label}</span>
              <ChevronDown size={13} />
            </button>
            {a.kind !== "trash" && a.kind !== "keep" && (
              <div className="dest">
                {(a.kind === "move" || a.kind === "copy" || a.kind === "shortcut" || a.kind === "extract") && (
                  <button
                    className="icon-btn"
                    title={t("选择文件夹")}
                    onClick={async () => {
                      const d = await pickFolder(t("选择目标文件夹"));
                      if (d) update(i, { to: d });
                    }}
                  >
                    <FolderOpen size={16} />
                  </button>
                )}
                <input
                  className="dest-input mono"
                  placeholder={PLACEHOLDER[a.kind]}
                  value={a.kind === "rename" || a.kind === "compress" ? a.to : tilde(a.to)}
                  title={a.to}
                  onChange={(e) => update(i, { to: e.target.value })}
                />
                <VarMenu onInsert={(v) => update(i, { to: a.to + v })} />
                <select
                  className="pill-select"
                  value={a.onConflict}
                  title={t("目标位置已有同名文件时")}
                  onChange={(e) => update(i, { onConflict: e.target.value as "rename" | "skip" })}
                >
                  <option value="rename">{t("同名保留两份")}</option>
                  <option value="skip">{t("同名跳过")}</option>
                </select>
              </div>
            )}
            {rows.length > 1 && (
              <button
                className="icon-btn"
                aria-label={t("删除这一步")}
                title={t("删除这一步")}
                onClick={() => onChange(rows.filter((_, j) => j !== i))}
              >
                <X size={15} />
              </button>
            )}
          </div>
        );
      })}
      {menu.node}
      <div className="add-row">
        <button className="add-btn" onClick={() => onChange([...rows, { kind: "move", to: "", onConflict: "rename" }])}>
          <Plus size={14} /> {t("再加一步")}
        </button>
      </div>
    </div>
  );
}
