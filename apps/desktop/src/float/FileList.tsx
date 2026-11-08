import {
  ArrowRight,
  ChevronDown,
  ChevronRight,
  FileType,
  FolderOpen,
  Layers,
  List,
  ListFilter,
  Trash,
  WandSparkles,
  X,
} from "lucide-react";
import { useMemo, useState, type MouseEvent } from "react";
import { baseName, type SkipView } from "../api";
import { Checkbox, FileIcon } from "../components/ui";
import { t } from "../i18n";
import { groupItems, pathKey, type FloatItem, type View } from "./items";

export default function FileList({
  items,
  skipped,
  excluded,
  view,
  onToggle,
  onRemove,
  onMenu,
  onNewRule,
}: {
  items: FloatItem[];
  skipped: SkipView[];
  excluded: Set<string>;
  view: View;
  onToggle: (paths: string[], on: boolean) => void;
  onRemove: (paths: string[]) => void;
  onMenu: (event: MouseEvent, path: string, group?: number, noMatch?: boolean) => void;
  onNewRule: (path: string) => void;
}) {
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const groups = useMemo(() => groupItems(items, view), [items, view]);
  const skippedGroups = [
    { key: "no-match", title: t("没有匹配规则的文件"), files: skipped.filter((file) => file.kind === "no_match") },
    { key: "kept", title: t("保持原位"), files: skipped.filter((file) => file.kind !== "no_match") },
  ].filter((group) => group.files.length);
  const keys = [...(view === "none" ? [] : groups.map((group) => group.key)), ...skippedGroups.map((group) => group.key)];
  const toggleCollapsed = (key: string) =>
    setCollapsed((previous) => {
      const next = new Set(previous);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  const row = (item: FloatItem) => {
    const on = !excluded.has(pathKey(item.path));
    return (
      <div
        key={item.group}
        className={`float-item ${on ? "" : "off"}`}
        title={t("规则「{0}」", [item.rule])}
        onClick={() => onToggle([item.path], !on)}
        onContextMenu={(event) => onMenu(event, item.path, item.group)}
      >
        <Checkbox checked={on} onChange={(checked) => onToggle([item.path], checked)} label={item.name} />
        <FileIcon category={item.category} name={item.name} size={26} />
        <span className="ellipsis name">{item.name}</span>
        <span className="float-to ellipsis">
          {item.kind === "trash" ? (
            <>
              <Trash size={12} /> {t("回收站")}
            </>
          ) : (
            <>
              <ArrowRight size={12} /> {baseName(item.dir ?? "")}
            </>
          )}
        </span>
        <button
          className="icon-btn float-remove"
          title={t("从列表中移除")}
          onClick={(event) => {
            event.stopPropagation();
            onRemove([item.path]);
          }}
        >
          <X size={13} />
        </button>
      </div>
    );
  };
  return (
    <>
      {view !== "none" && (
        <div className="float-list-tools">
          <button
            className="icon-btn"
            disabled={!keys.length}
            title={collapsed.size ? t("展开全部") : t("折叠全部")}
            onClick={() => setCollapsed(collapsed.size ? new Set() : new Set(keys))}
          >
            {collapsed.size ? <ChevronDown size={15} /> : <ChevronRight size={15} />}
          </button>
        </div>
      )}
      {groups.map((group) => {
        if (view === "none") return <div key={group.key}>{group.items.map(row)}</div>;
        const count = group.items.filter((item) => !excluded.has(pathKey(item.path))).length;
        return (
          <section className="float-group" key={group.key}>
            <div className="float-group-head" title={t("取消勾选可排除此组")}>
              <Checkbox
                checked={count === group.items.length}
                indeterminate={count > 0}
                onChange={(on) =>
                  onToggle(
                    group.items.map((item) => item.path),
                    on,
                  )
                }
                label={group.title}
              />
              <button className="float-group-toggle" aria-expanded={!collapsed.has(group.key)} onClick={() => toggleCollapsed(group.key)}>
                {collapsed.has(group.key) ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                <span className="ellipsis">{group.title}</span>
                <span className="spacer" />
                <span className="float-group-count">
                  {count}/{group.items.length}
                </span>
              </button>
            </div>
            {!collapsed.has(group.key) && group.items.map(row)}
          </section>
        );
      })}
      {skippedGroups.map((group) => (
        <section className={view === "none" ? "" : "float-group"} key={group.key}>
          {view !== "none" && (
            <div className="float-group-head">
              <button className="float-group-toggle" aria-expanded={!collapsed.has(group.key)} onClick={() => toggleCollapsed(group.key)}>
                {collapsed.has(group.key) ? <ChevronRight size={14} /> : <ChevronDown size={14} />}
                <span className="ellipsis">{group.title}</span>
                <span className="spacer" />
                <span className="float-group-count">{group.files.length}</span>
              </button>
              <button className="icon-btn" title={t("从列表中移除此组")} onClick={() => onRemove(group.files.map((file) => file.path))}>
                <X size={13} />
              </button>
            </div>
          )}
          {(view === "none" || !collapsed.has(group.key)) &&
            group.files.map((file) => (
              <div
                key={file.path}
                className="float-item skip"
                tabIndex={file.kind === "no_match" ? 0 : undefined}
                title={file.reason}
                onContextMenu={(event) => onMenu(event, file.path, undefined, file.kind === "no_match")}
              >
                <FileIcon category={file.kind === "folder" ? "文件夹" : null} name={file.name} size={26} />
                <span className="ellipsis name">{file.name}</span>
                <span className="float-to ellipsis">{file.label}</span>
                {file.kind === "no_match" && (
                  <button className="btn sm float-add-rule" title={t("为此文件添加规则")} onClick={() => onNewRule(file.path)}>
                    <WandSparkles size={12} />
                    {t("添加规则")}
                  </button>
                )}
                <button className="icon-btn float-remove" title={t("从列表中移除")} onClick={() => onRemove([file.path])}>
                  <X size={13} />
                </button>
              </div>
            ))}
        </section>
      ))}
    </>
  );
}

export function ViewSwitcher({ view, onView }: { view: View; onView: (view: View) => void }) {
  const views = [
    { value: "none", label: t("列表"), Icon: List },
    { value: "category", label: t("按类型分组"), Icon: Layers },
    { value: "extension", label: t("按扩展名分组"), Icon: FileType },
    { value: "target", label: t("按目标位置分组"), Icon: FolderOpen },
    { value: "rule", label: t("按规则分组"), Icon: ListFilter },
  ] as const;
  return (
    <div className="segmented float-view-switch" role="group" aria-label={t("文件视图")}>
      {views.map(({ value, label, Icon }) => (
        <button
          key={value}
          className={view === value ? "on" : ""}
          title={label}
          aria-label={label}
          aria-pressed={view === value}
          onClick={() => onView(view === value ? "none" : value)}
        >
          <Icon size={15} />
        </button>
      ))}
    </div>
  );
}
