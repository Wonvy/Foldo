import {
  ArrowRight,
  ChevronDown,
  ChevronRight,
  CircleCheck,
  Ellipsis,
  FileSearch,
  Folder,
  FolderInput,
  FolderPlus,
  Lock,
  MousePointerClick,
  RefreshCw,
  RotateCcw,
  Search,
  Sparkles,
  Trash,
  TriangleAlert,
  Upload,
  WandSparkles,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import {
  api,
  baseName,
  errorText,
  type ExecView,
  type FolderSuggestion,
  type OpView,
  type PlanView,
  type SkipView,
  type UndoView,
} from "../api";
import InspectDialog from "../components/InspectDialog";
import UndoDialog from "../components/UndoDialog";
import { Checkbox, FileIcon, Segmented, Spinner, useMenu } from "../components/ui";
import { pickFolder, useStore } from "../store";

type Item = {
  group: number;
  origin: string;
  name: string;
  originDir: string;
  category: string | null;
  reason: string;
  kind: "move" | "copy" | "rename" | "trash";
  dir: string | null;
  dirDisplay: string | null;
  renamedTo: string | null;
  note: string | null;
};

type Section = { key: string; title: string; sub: string | null; items: Item[]; trash?: boolean };

const ATTENTION = new Set(["conflict", "invalid", "in_use"]);

function toItems(ops: OpView[]): Item[] {
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
    };
  });
}

export default function TidyPage({
  dropped,
  scanRequest,
  onNewRule,
}: {
  dropped: { paths: string[]; n: number } | null;
  /** 每次变化都重新扫描（托盘“立即整理”）。 */
  scanRequest: number;
  /** 跳到规则页并打开“新建规则”。 */
  onNewRule: () => void;
}) {
  const { view, config, save, setView } = useStore();
  const folders = config.folders ?? [];
  const configured = !!view?.exists && folders.length > 0;

  const [plan, setPlan] = useState<PlanView | null>(null);
  const [dropPaths, setDropPaths] = useState<string[] | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState<null | "scan" | "exec">(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ExecView | null>(null);
  const [undoFor, setUndoFor] = useState<number | null>(null);
  const [undone, setUndone] = useState<UndoView | null>(null);
  const [groupBy, setGroupBy] = useState<"target" | "rule">("target");
  const [confirmTrash, setConfirmTrash] = useState(false);
  const [showKept, setShowKept] = useState(false);
  const [inspectPath, setInspectPath] = useState<string | null>(null);
  const menu = useMenu();

  const applyPlan = (p: PlanView) => {
    setPlan(p);
    setSelected(new Set(p.ops.map((o) => o.group)));
    setResult(null);
    setUndone(null);
    setConfirmTrash(false);
  };

  const run = useCallback(async (paths: string[] | null) => {
    setBusy("scan");
    setError(null);
    try {
      const p = paths ? await api.scanPaths(paths) : await api.scan();
      setDropPaths(paths);
      applyPlan(p);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  }, []);

  // 打开应用时自动扫描一次
  const autoScanned = useRef(false);
  useEffect(() => {
    if (configured && !autoScanned.current) {
      autoScanned.current = true;
      void run(null);
    }
  }, [configured, run]);

  // 托盘里点了“立即整理”
  useEffect(() => {
    if (scanRequest && configured) void run(null);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [scanRequest]);

  // 拖入文件
  useEffect(() => {
    if (dropped) void run(dropped.paths);
  }, [dropped, run]);

  const items = useMemo(() => (plan ? toItems(plan.ops) : []), [plan]);
  const sections = useMemo<Section[]>(() => {
    const m = new Map<string, Section>();
    for (const it of items) {
      let key: string;
      let s: Omit<Section, "items">;
      if (groupBy === "rule") {
        key = it.reason;
        s = { key, title: it.reason, sub: null };
      } else if (it.kind === "trash") {
        key = "trash";
        s = { key, title: "移到回收站", sub: "撤销时需要到回收站手动还原", trash: true };
      } else {
        key = `${it.kind}:${it.dir}`;
        s = {
          key,
          title: `${it.kind === "copy" ? "复制到 " : ""}${baseName(it.dir ?? "")}`,
          sub: it.dirDisplay,
        };
      }
      if (!m.has(key)) m.set(key, { ...s, items: [] });
      m.get(key)!.items.push(it);
    }
    return [...m.values()];
  }, [items, groupBy]);

  const skipped = plan?.skipped ?? [];
  const noMatch = skipped.filter((s) => s.kind === "no_match");
  const attention = skipped.filter((s) => ATTENTION.has(s.kind));
  const kept = skipped.filter((s) => s.kind !== "no_match" && !ATTENTION.has(s.kind));
  const keptByLabel = useMemo(() => {
    const m = new Map<string, SkipView[]>();
    for (const s of kept) m.set(s.label, [...(m.get(s.label) ?? []), s]);
    return [...m.entries()];
  }, [kept]);

  const selectedItems = items.filter((i) => selected.has(i.group));
  const trashSelected = selectedItems.some((i) => i.kind === "trash");

  const toggle = (groups: number[], on: boolean) =>
    setSelected((prev) => {
      const next = new Set(prev);
      for (const g of groups) {
        if (on) next.add(g);
        else next.delete(g);
      }
      return next;
    });

  const execute = async () => {
    if (!plan) return;
    if (trashSelected && !confirmTrash) {
      setConfirmTrash(true);
      return;
    }
    setBusy("exec");
    setError(null);
    try {
      setResult(await api.executePlan(plan.plan_id, [...selected]));
      setPlan(null);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
      setConfirmTrash(false);
    }
  };

  const retarget = async (it: Item) => {
    if (!plan) return;
    const dir = await pickFolder(`把「${it.name}」放到…`);
    if (!dir) return;
    try {
      setPlan(await api.retarget(plan.plan_id, it.group, dir));
    } catch (e) {
      setError(errorText(e));
    }
  };

  const protect = async (path: string, group?: number) => {
    try {
      setView(await api.addProtect(path));
      setPlan((p) =>
        p && {
          ...p,
          ops: p.ops.filter((o) => o.group !== group),
          skipped: [
            ...p.skipped.filter((s) => s.path !== path),
            {
              path,
              name: baseName(path),
              display: path,
              kind: "protected",
              label: "固定保留",
              reason: "在固定保留列表中",
              rule: null,
            },
          ],
        },
      );
    } catch (e) {
      setError(errorText(e));
    }
  };

  const itemMenu = (el: HTMLElement, it: Item) =>
    menu.open(el, [
      { label: "这次改到其他位置…", icon: <FolderInput size={15} />, onClick: () => retarget(it) },
      { label: "固定保留（以后都不动）", icon: <Lock size={15} />, onClick: () => protect(it.origin, it.group) },
      "divider",
      { label: "查看文件属性", icon: <FileSearch size={15} />, onClick: () => setInspectPath(it.origin) },
      { label: "在文件管理器中显示", icon: <Search size={15} />, onClick: () => api.reveal(it.origin) },
    ]);

  const skipMenu = (el: HTMLElement, s: SkipView) =>
    menu.open(el, [
      { label: "固定保留（以后都不动）", icon: <Lock size={15} />, onClick: () => protect(s.path) },
      { label: "查看文件属性", icon: <FileSearch size={15} />, onClick: () => setInspectPath(s.path) },
      { label: "在文件管理器中显示", icon: <Search size={15} />, onClick: () => api.reveal(s.path) },
    ]);

  // ---------- 首次使用 ----------
  if (view && !configured) return <Onboarding onDone={() => void run(null)} />;

  const addFolder = async () => {
    const dir = await pickFolder("添加要整理的文件夹");
    if (!dir || folders.some((f) => f.path === dir)) return;
    try {
      await save({ ...config, folders: [...folders, { path: dir }] });
      void run(null);
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>整理</h1>
          <p className="sub">按你的规则给出整理建议，勾选后再执行；执行过的整理随时可以撤销。也可以把文件直接拖进窗口。</p>
        </div>
        <button className="btn primary" onClick={() => run(null)} disabled={busy !== null}>
          {busy === "scan" ? <Spinner size={15} /> : <RefreshCw size={15} />}
          {plan || result ? "重新扫描" : "扫描"}
        </button>
      </div>

      <div className="folder-bar">
        {folders.map((f) => (
          <span key={f.path} className="folder-chip" title={f.path}>
            <Folder size={14} />
            {baseName(f.path)}
          </span>
        ))}
        <button className="btn ghost sm" onClick={addFolder}>
          <FolderPlus size={15} /> 添加文件夹
        </button>
      </div>

      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          <span className="selectable">{error}</span>
        </div>
      )}

      {dropPaths && plan && (
        <div className="banner info">
          <Upload size={16} />
          <span style={{ flex: 1 }}>正在按你的规则处理拖入的 {dropPaths.length} 个项目。</span>
          <button className="link" onClick={() => run(null)}>
            返回整理文件夹
          </button>
        </div>
      )}

      {(config.rules ?? []).length === 0 && (
        <div className="banner info">
          <WandSparkles size={16} />
          <span style={{ flex: 1 }}>还没有规则。Foldo 只会按你建的规则移动文件，其余文件保持原位。</span>
          <button className="btn sm primary" onClick={onNewRule}>
            新建规则
          </button>
        </div>
      )}

      {plan?.warnings.map((w) => (
        <div key={w} className="banner warn">
          <TriangleAlert size={16} />
          {w}
        </div>
      ))}

      {/* ---------- 执行结果 ---------- */}
      {result && (
        <div className="card">
          <div className="result-hero">
            <div className={`art ${result.failed.length ? "warn" : ""}`}>
              {result.failed.length ? <TriangleAlert size={26} /> : <CircleCheck size={28} />}
            </div>
            <h2>
              {undone
                ? `已撤销，恢复了 ${undone.restored} 项`
                : `整理完成：成功 ${result.done} 项`}
            </h2>
            <p className="muted">
              {result.skipped.length > 0 && `跳过 ${result.skipped.length} 项（执行时出现了新冲突）。`}
              {result.failed.length > 0 && `失败 ${result.failed.length} 项。`}
              {!undone && result.skipped.length + result.failed.length === 0 && "所有选中的文件都已放到位。"}
            </p>
            <div className="row" style={{ marginTop: 10 }}>
              {!undone && result.done > 0 && (
                <button className="btn" onClick={() => setUndoFor(result.batch_id)}>
                  <RotateCcw size={15} /> 撤销这次整理
                </button>
              )}
              <button className="btn primary" onClick={() => run(null)}>
                继续整理
              </button>
            </div>
          </div>
          {[...result.skipped.map((p) => ({ ...p, t: "跳过" })), ...result.failed.map((p) => ({ ...p, t: "失败" }))].map(
            (p, i) => (
              <div key={i} className="list-row">
                <span className={`chip ${p.t === "失败" ? "danger" : "warn"}`}>{p.t}</span>
                <div className="row-main">
                  <div className="row-title ellipsis">{p.name}</div>
                  <div className="row-sub">{p.error}</div>
                </div>
              </div>
            ),
          )}
          {undone?.in_trash.map((p) => (
            <div key={p} className="list-row">
              <span className="chip">回收站</span>
              <div className="row-main">
                <div className="row-sub">需要到回收站手动还原：{p}</div>
              </div>
            </div>
          ))}
          <div style={{ height: 12 }} />
        </div>
      )}

      {!plan && !result && busy === "scan" && (
        <div className="empty muted">
          <Spinner size={22} />
          正在扫描…
        </div>
      )}

      {/* ---------- 整理建议 ---------- */}
      {plan && (
        <>
          <div className="stats">
            <div className="stat">
              <div className="num">{items.length}</div>
              <div className="label">个文件有整理建议</div>
            </div>
            <button className="stat" onClick={() => document.getElementById("no-match")?.scrollIntoView({ behavior: "smooth" })}>
              <div className="num">{noMatch.length}</div>
              <div className="label">个文件没有匹配的规则</div>
            </button>
            <button
              className="stat"
              onClick={() => {
                setShowKept(true);
                setTimeout(() => document.getElementById("kept")?.scrollIntoView({ behavior: "smooth" }), 0);
              }}
            >
              <div className="num">{kept.length + attention.length}</div>
              <div className="label">项保持原位</div>
            </button>
          </div>

          {attention.length > 0 && (
            <div className="card">
              <div className="card-head">
                <TriangleAlert size={16} color="var(--warn)" />
                <h2>需要留意</h2>
                <span className="muted">这些文件本次不会移动</span>
              </div>
              {attention.map((s) => (
                <SkipRow key={s.path} s={s} onMenu={(el) => skipMenu(el, s)} />
              ))}
            </div>
          )}

          {items.length > 0 && (
            <div className="toolbar">
              <Checkbox
                checked={selected.size === items.length}
                indeterminate={selected.size > 0}
                onChange={(on) => toggle(items.map((i) => i.group), on)}
                label="全选"
              />
              <span className="text-2">全选</span>
              <span className="spacer" />
              <Segmented
                value={groupBy}
                onChange={setGroupBy}
                options={[
                  { value: "target", label: "按目标位置" },
                  { value: "rule", label: "按整理原因" },
                ]}
              />
            </div>
          )}

          {sections.map((sec) => {
            const groups = sec.items.map((i) => i.group);
            const on = groups.filter((g) => selected.has(g)).length;
            return (
              <div key={sec.key} className="card">
                <div className="group-head">
                  <Checkbox
                    checked={on === groups.length}
                    indeterminate={on > 0}
                    onChange={(v) => toggle(groups, v)}
                    label={sec.title}
                  />
                  {sec.trash ? <Trash size={16} color="var(--danger)" /> : <Folder size={16} color="var(--muted)" />}
                  <span className="title">{sec.title}</span>
                  {sec.sub && <span className="muted ellipsis mono" style={{ flex: 1 }}>{sec.sub}</span>}
                  {!sec.sub && <span className="spacer" />}
                  <span className="muted">{sec.items.length} 个</span>
                </div>
                {sec.items.map((it) => (
                  <div
                    key={it.group}
                    className={`list-row clickable ${selected.has(it.group) ? "" : "off"}`}
                    onClick={() => toggle([it.group], !selected.has(it.group))}
                  >
                    <Checkbox checked={selected.has(it.group)} onChange={(v) => toggle([it.group], v)} label={it.name} />
                    <FileIcon category={it.category} />
                    <div className="row-main">
                      <div className="row-title ellipsis">
                        {it.name}
                        {it.renamedTo && (
                          <span className="muted" style={{ fontWeight: 400 }}>
                            {"  "}→ 改名为 {it.renamedTo}
                          </span>
                        )}
                      </div>
                      <div className="item-dst">
                        <span className={`chip ${it.reason === "手动指定位置" ? "accent" : ""}`}>{it.reason}</span>
                        {groupBy === "rule" && it.kind !== "trash" && (
                          <>
                            <ArrowRight size={13} />
                            <span className="ellipsis">{it.dirDisplay}</span>
                          </>
                        )}
                        {groupBy === "target" && <span className="ellipsis">来自 {it.originDir}</span>}
                        {it.note && <span className="chip warn">{it.note}</span>}
                      </div>
                    </div>
                    <button
                      className="icon-btn"
                      aria-label="更多"
                      onClick={(e) => {
                        e.stopPropagation();
                        itemMenu(e.currentTarget, it);
                      }}
                    >
                      <Ellipsis size={17} />
                    </button>
                  </div>
                ))}
              </div>
            );
          })}

          {noMatch.length > 0 && (
            <div className="card" id="no-match">
              <div className="card-head">
                <Sparkles size={16} color="var(--accent)" />
                <h2>没有匹配规则的文件</h2>
                <span className="muted">这些文件保持原位</span>
                <span className="spacer" />
                <button className="btn sm" onClick={onNewRule}>
                  <WandSparkles size={14} /> 新建规则
                </button>
              </div>
              {noMatch.map((s) => (
                <div key={s.path} className="list-row">
                  <FileIcon category={null} />
                  <div className="row-main">
                    <div className="row-title ellipsis">{s.name}</div>
                    <div className="row-sub ellipsis">{s.display}</div>
                  </div>
                  <button className="icon-btn" aria-label="更多" onClick={(e) => skipMenu(e.currentTarget, s)}>
                    <Ellipsis size={17} />
                  </button>
                </div>
              ))}
            </div>
          )}

          {kept.length > 0 && (
            <div className="card" id="kept">
              <button
                className="card-head path-link"
                style={{ width: "100%", borderBottom: showKept ? undefined : 0, textDecoration: "none", color: "inherit" }}
                onClick={() => setShowKept(!showKept)}
              >
                {showKept ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                <h2>保持原位的项目</h2>
                <span className="muted ellipsis">
                  {keptByLabel.map(([label, list]) => `${label} ${list.length}`).join(" · ")}
                </span>
              </button>
              {showKept &&
                keptByLabel.map(([label, list]) => (
                  <div key={label}>
                    <div className="section-title" style={{ padding: "12px 16px 4px" }}>
                      {label}（{list.length}）
                    </div>
                    {list.map((s) => (
                      <SkipRow key={s.path} s={s} onMenu={(el) => skipMenu(el, s)} />
                    ))}
                  </div>
                ))}
            </div>
          )}

          {items.length === 0 && noMatch.length === 0 && (config.rules ?? []).length > 0 && (
            <div className="empty">
              <div className="art">
                <CircleCheck size={26} />
              </div>
              <h2>已经很整齐了</h2>
              <p className="muted">没有需要整理的文件。</p>
            </div>
          )}

          {items.length > 0 && (
            <div className="footer-bar">
              <span className="text-2">
                已选 <b>{selected.size}</b> / {items.length} 个文件
              </span>
              <span className="spacer" />
              {confirmTrash && (
                <span className="chip warn" style={{ height: 30 }}>
                  <TriangleAlert size={14} />
                  选中的文件里有要移到回收站的，确定继续？
                </span>
              )}
              {confirmTrash && (
                <button className="btn" onClick={() => setConfirmTrash(false)}>
                  取消
                </button>
              )}
              <button
                className={`btn lg ${confirmTrash ? "danger" : "primary"}`}
                disabled={selected.size === 0 || busy !== null}
                onClick={execute}
              >
                {busy === "exec" ? <Spinner size={15} /> : <MousePointerClick size={16} />}
                {confirmTrash ? "确定执行" : `整理选中的 ${selected.size} 个文件`}
              </button>
            </div>
          )}
        </>
      )}

      {menu.node}
      {inspectPath && <InspectDialog path={inspectPath} onClose={() => setInspectPath(null)} />}
      {undoFor !== null && (
        <UndoDialog
          batchId={undoFor}
          onClose={() => setUndoFor(null)}
          onDone={(r) => {
            setUndoFor(null);
            setUndone(r);
          }}
        />
      )}
    </div>
  );
}

function SkipRow({ s, onMenu }: { s: SkipView; onMenu: (el: HTMLElement) => void }) {
  return (
    <div className="list-row">
      <FileIcon category={null} size={28} />
      <div className="row-main">
        <div className="row-title ellipsis">{s.name}</div>
        <div className="row-sub ellipsis">{s.reason}</div>
      </div>
      <button className="icon-btn" aria-label="更多" onClick={(e) => onMenu(e.currentTarget)}>
        <Ellipsis size={17} />
      </button>
    </div>
  );
}

// ---------- 首次使用：选择要整理的文件夹 ----------

function Onboarding({ onDone }: { onDone: () => void }) {
  const { config, save } = useStore();
  const [suggestions, setSuggestions] = useState<FolderSuggestion[]>([]);
  const [chosen, setChosen] = useState<string[]>([]);
  const [extra, setExtra] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .suggestedFolders()
      .then((s) => {
        setSuggestions(s);
        setChosen(s.filter((x) => x.label !== "文稿").map((x) => x.path));
      })
      .catch(() => setSuggestions([]));
  }, []);

  const all = [...suggestions.map((s) => ({ path: s.path, title: s.label, sub: s.display })), ...extra.map((p) => ({ path: p, title: baseName(p), sub: p }))];

  const start = async () => {
    setSaving(true);
    setError(null);
    try {
      await save({
        ...config,
        version: 1,
        folders: chosen.map((path) => ({ path })),
        settings: config.settings ?? { buffer: "10m" },
      });
      onDone();
    } catch (e) {
      setError(errorText(e));
      setSaving(false);
    }
  };

  return (
    <div className="page">
      <div className="empty" style={{ maxWidth: 520 }}>
        <div className="art">
          <Sparkles size={26} />
        </div>
        <h1>选择要整理的文件夹</h1>
        <p className="muted">
          Foldo 只整理这些文件夹第一层的散落文件，已有的文件夹会整体保留。之后在“规则”里写下哪些文件放到哪里，先看建议再执行，随时可以撤销。
        </p>
        <div className="card" style={{ width: "100%", marginTop: 14, textAlign: "left" }}>
          {all.map((f) => (
            <label key={f.path} className="list-row clickable">
              <Checkbox
                checked={chosen.includes(f.path)}
                onChange={(on) => setChosen(on ? [...chosen, f.path] : chosen.filter((p) => p !== f.path))}
                label={f.title}
              />
              <Folder size={18} color="var(--accent)" />
              <div className="row-main">
                <div className="row-title">{f.title}</div>
                <div className="row-sub mono">{f.sub}</div>
              </div>
            </label>
          ))}
          <div className="list-row">
            <button
              className="btn ghost sm"
              onClick={async () => {
                const d = await pickFolder("选择要整理的文件夹");
                if (d && !all.some((f) => f.path === d)) {
                  setExtra([...extra, d]);
                  setChosen([...chosen, d]);
                }
              }}
            >
              <FolderPlus size={15} /> 选择其他文件夹…
            </button>
          </div>
        </div>
        {error && <div className="banner error">{error}</div>}
        <button className="btn primary lg" style={{ marginTop: 10 }} disabled={chosen.length === 0 || saving} onClick={start}>
          {saving ? <Spinner size={15} /> : <ArrowRight size={16} />}
          开始整理
        </button>
      </div>
    </div>
  );
}
