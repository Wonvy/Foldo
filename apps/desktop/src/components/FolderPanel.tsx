// 整理页顶部的文件夹列表：每个文件夹添加 / 移除规则，设置是否含子文件夹和隐藏文件。
import { Check, Ellipsis, Folder, FolderPlus, Plus, Search, Trash, WandSparkles, X } from "lucide-react";
import { useState } from "react";
import { api, baseName, errorText, type FolderDef, type RuleFile, type WatchStatus } from "../api";
import { tilde } from "../rules";
import { pickFolder, useStore } from "../store";
import { useMenu, type MenuItem } from "./ui";

export default function FolderPanel({
  watch,
  onAdded,
  onEditRule,
  onNewRule,
}: {
  watch: WatchStatus | null;
  /** 添加文件夹或改了规则之后（用来重新扫描）。 */
  onAdded: () => void;
  onEditRule: (name: string) => void;
  /** 新建规则，保存后加入这个文件夹。 */
  onNewRule: (folder: string) => void;
}) {
  const { config, save } = useStore();
  const folders = config.folders ?? [];
  const rules = config.rules ?? [];
  const [error, setError] = useState<string | null>(null);
  const menu = useMenu();

  const update = async (next: RuleFile) => {
    setError(null);
    try {
      await save(next);
      onAdded();
    } catch (e) {
      setError(errorText(e));
    }
  };
  const setFolder = (i: number, patch: Partial<FolderDef>) =>
    update({ ...config, folders: folders.map((f, j) => (j === i ? { ...f, ...patch } : f)) });

  const addFolder = async () => {
    const dir = await pickFolder("添加要整理的文件夹");
    if (!dir || folders.some((f) => f.path === dir)) return;
    await update({ ...config, folders: [...folders, { path: dir, rules: [] }] });
  };

  // 按规则列表的顺序显示（也是执行顺序）
  const used = (f: FolderDef) => rules.filter((r) => f.rules?.includes(r.name));

  const addRuleMenu = (el: HTMLElement, i: number) => {
    const f = folders[i];
    const rest = rules.filter((r) => !f.rules?.includes(r.name));
    const items: MenuItem[] = rest.map((r) => ({
      label: r.enabled === false ? `${r.name}（已停用）` : r.name,
      icon: <WandSparkles size={15} />,
      onClick: () => setFolder(i, { rules: [...(f.rules ?? []), r.name] }),
    }));
    if (items.length) items.push("divider");
    items.push({ label: "新建规则…", icon: <Plus size={15} />, onClick: () => onNewRule(f.path) });
    menu.open(el, items);
  };

  const folderMenu = (el: HTMLElement, i: number) => {
    const f = folders[i];
    const tick = (on: boolean | undefined) => (on ? <Check size={15} /> : <span style={{ width: 15 }} />);
    menu.open(el, [
      { label: "包含子文件夹", icon: tick(f.recursive), onClick: () => setFolder(i, { recursive: f.recursive ? undefined : true }) },
      {
        label: "包含隐藏文件",
        icon: tick(f.include_hidden),
        onClick: () => setFolder(i, { include_hidden: f.include_hidden ? undefined : true }),
      },
      "divider",
      { label: "在文件管理器中显示", icon: <Search size={15} />, onClick: () => api.reveal(f.path) },
      {
        label: "移除文件夹",
        icon: <Trash size={15} />,
        danger: true,
        onClick: () => update({ ...config, folders: folders.filter((_, j) => j !== i) }),
      },
    ]);
  };

  const live = watch && watch.mode !== "off";

  return (
    <div className="card folder-panel">
      {error && <div className="banner error selectable" style={{ margin: 12 }}>{error}</div>}
      {folders.map((f, i) => {
        const list = used(f);
        const watching = live && list.some((r) => r.enabled !== false);
        return (
          <div key={f.path} className="folder-row">
            <div className="folder-name" title={tilde(f.path)}>
              <Folder size={17} color="var(--accent)" />
              <div className="row-main">
                <div className="row-title ellipsis">
                  {baseName(f.path)}
                  {watching && <span className="dot-live" title="正在监听" />}
                </div>
                <div className="row-sub ellipsis">
                  {[f.recursive && "含子文件夹", f.include_hidden && "含隐藏文件"].filter(Boolean).join(" · ") || tilde(f.path)}
                </div>
              </div>
            </div>
            <div className="folder-rules">
              {list.length === 0 && <span className="muted" style={{ fontSize: 12.5 }}>还没有规则，这里的文件不会被整理</span>}
              {list.map((r) => (
                <span key={r.name} className={`rule-chip ${r.enabled === false ? "off" : ""}`} title={r.enabled === false ? "已停用" : "编辑规则"}>
                  <button className="rule-chip-name" onClick={() => onEditRule(r.name)}>
                    {r.name}
                  </button>
                  <button
                    className="rule-chip-x"
                    aria-label={`从 ${baseName(f.path)} 移除 ${r.name}`}
                    onClick={() => setFolder(i, { rules: (f.rules ?? []).filter((n) => n !== r.name) })}
                  >
                    <X size={12} />
                  </button>
                </span>
              ))}
              <button className="add-btn" onClick={(e) => addRuleMenu(e.currentTarget, i)}>
                <Plus size={14} /> 规则
              </button>
            </div>
            <button className="icon-btn" aria-label="文件夹选项" onClick={(e) => folderMenu(e.currentTarget, i)}>
              <Ellipsis size={16} />
            </button>
          </div>
        );
      })}
      <button className="folder-add" onClick={addFolder}>
        <FolderPlus size={15} /> 添加文件夹
      </button>
      {menu.node}
    </div>
  );
}
