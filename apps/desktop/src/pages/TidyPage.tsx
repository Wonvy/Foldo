import { CircleCheck, FileSearch, FolderInput, Lock, MousePointerClick, RefreshCw, Search, TriangleAlert, Upload } from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { api, baseName, errorText, type ExecView, type PlanView, type SkipView, type UndoView, type WatchStatus } from "../api";
import AutoMode from "../components/AutoMode";
import FolderPanel from "../components/FolderPanel";
import SetupGuide, { markSetupDone, setupDone } from "../components/SetupGuide";
import InspectDialog from "../components/InspectDialog";
import ItemSections from "../components/tidy/ItemSections";
import { ATTENTION, SECTION_PREFIX, toItems, type Item, type Section } from "../components/tidy/items";
import KeptCard from "../components/tidy/KeptCard";
import NoMatchCard from "../components/tidy/NoMatchCard";
import Onboarding from "../components/tidy/Onboarding";
import ResultCard from "../components/tidy/ResultCard";
import SkipRow from "../components/tidy/SkipRow";
import UndoDialog from "../components/UndoDialog";
import { Checkbox, Segmented, Spinner, useMenu } from "../components/ui";
import { pickFolder, useStore } from "../store";
import { t } from "../i18n";

export default function TidyPage({
  dropped,
  scanRequest,
  watch,
  onNewRule,
  onEditRule,
}: {
  dropped: { paths: string[]; n: number } | null;
  /** 每次变化都重新扫描（托盘“立即整理”）。 */
  scanRequest: number;
  watch: WatchStatus | null;
  /** 跳到规则页并打开“新建规则”；指定文件夹时保存后加入这个文件夹。 */
  onNewRule: (folder?: string) => void;
  onEditRule: (name: string) => void;
}) {
  const { view, config, setView } = useStore();
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
  const [expandedExtensions, setExpandedExtensions] = useState<Set<string>>(new Set());
  const [inspectPath, setInspectPath] = useState<string | null>(null);
  const [guideDismissed, setGuideDismissed] = useState(setupDone);
  const rulesInUse = (config.folders ?? []).some((f) => (config.rules ?? []).some((r) => r.enabled !== false && f.rules?.includes(r.name)));
  const showGuide = !rulesInUse || !guideDismissed;
  const menu = useMenu();

  const applyPlan = (p: PlanView) => {
    setPlan(p);
    setExpandedExtensions(new Set());
    setSelected(new Set(p.ops.map((o) => o.group)));
    setResult(null);
    setUndone(null);
    setConfirmTrash(false);
  };

  // 用户点了“取消”：扫描返回的“已取消”不当作错误
  const cancelled = useRef(false);
  const run = useCallback(async (paths: string[] | null) => {
    setBusy("scan");
    setError(null);
    cancelled.current = false;
    try {
      const p = paths ? await api.scanPaths(paths) : await api.scan();
      setDropPaths(paths);
      applyPlan(p);
    } catch (e) {
      if (!cancelled.current) setError(errorText(e));
    } finally {
      setBusy(null);
    }
  }, []);

  // 扫描大文件夹时显示进度
  const [progress, setProgress] = useState<{ found: number; checked: number } | null>(null);
  useEffect(() => {
    if (busy !== "scan") {
      setProgress(null);
      return;
    }
    const timer = setInterval(() => {
      api
        .scanProgress()
        .then(setProgress)
        .catch(() => {});
    }, 300);
    return () => clearInterval(timer);
  }, [busy]);

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
        s = { key, title: t("移到回收站"), sub: t("撤销时需要到回收站手动还原"), trash: true };
      } else {
        key = `${it.kind}:${it.dir}`;
        s = {
          key,
          title: `${SECTION_PREFIX[it.kind] ?? ""}${baseName(it.dir ?? "")}`,
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
  const noMatchByExtension = useMemo(() => {
    const groups = new Map<string, SkipView[]>();
    for (const file of plan?.skipped ?? []) {
      if (file.kind !== "no_match") continue;
      const dot = file.name.lastIndexOf(".");
      const extension = dot > 0 && dot < file.name.length - 1 ? file.name.slice(dot).toLowerCase() : "";
      const group = groups.get(extension);
      if (group) group.push(file);
      else groups.set(extension, [file]);
    }
    return [...groups.entries()].sort(([a], [b]) => {
      if (!a) return 1;
      if (!b) return -1;
      return a.localeCompare(b);
    });
  }, [plan]);
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
      // 第一次执行过整理，上手引导就完成了
      markSetupDone();
      setGuideDismissed(true);
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
    const dir = await pickFolder(t("把「{0}」放到…", [it.name]));
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
      setPlan(
        (p) =>
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
                label: t("固定保留"),
                reason: t("在固定保留列表中"),
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
      { label: t("这次改到其他位置…"), icon: <FolderInput size={15} />, onClick: () => retarget(it) },
      { label: t("固定保留（以后都不动）"), icon: <Lock size={15} />, onClick: () => protect(it.origin, it.group) },
      "divider",
      { label: t("查看文件属性"), icon: <FileSearch size={15} />, onClick: () => setInspectPath(it.origin) },
      { label: t("在文件管理器中显示"), icon: <Search size={15} />, onClick: () => api.reveal(it.origin) },
    ]);

  const skipMenu = (el: HTMLElement, s: SkipView) =>
    menu.open(el, [
      { label: t("固定保留（以后都不动）"), icon: <Lock size={15} />, onClick: () => protect(s.path) },
      { label: t("查看文件属性"), icon: <FileSearch size={15} />, onClick: () => setInspectPath(s.path) },
      { label: t("在文件管理器中显示"), icon: <Search size={15} />, onClick: () => api.reveal(s.path) },
    ]);

  // ---------- 首次使用 ----------
  // 有配置方案时，空方案也显示文件夹面板（可以切换回去或添加文件夹），不进入首次引导
  if (view && !configured && !config.profiles?.length) return <Onboarding onDone={() => void run(null)} />;

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{t("整理")}</h1>
          <p className="sub">{t("每个文件夹按添加的规则整理，先预览再执行，随时可以撤销。")}</p>
        </div>
        <div className="page-actions">
          <AutoMode watch={watch} />
          {busy === "scan" && progress && progress.found > 0 && (
            <span className="muted scan-progress">{t("已找到 {0} 个，已检查 {1} 个", [progress.found, progress.checked])}</span>
          )}
          {busy === "scan" && !dropPaths && (
            <button
              className="btn"
              onClick={() => {
                cancelled.current = true;
                void api.cancelScan();
              }}
            >
              {t("取消")}
            </button>
          )}
          <button data-testid="scan" className="btn primary" onClick={() => run(null)} disabled={busy !== null}>
            {busy === "scan" ? <Spinner size={15} /> : <RefreshCw size={15} />}
            {plan || result ? t("重新扫描") : t("扫描")}
          </button>
        </div>
      </div>

      <FolderPanel watch={watch} onAdded={() => void run(null)} onEditRule={onEditRule} onNewRule={onNewRule} />

      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          <span className="selectable">{error}</span>
        </div>
      )}

      {dropPaths && plan && (
        <div className="banner info">
          <Upload size={16} />
          <span style={{ flex: 1 }}>{t("正在按你的规则处理拖入的 {0} 个项目。", [dropPaths.length])}</span>
          <button className="link" onClick={() => run(null)}>
            {t("返回整理文件夹")}
          </button>
        </div>
      )}

      {showGuide && !dropPaths && (
        <SetupGuide
          onChanged={() => void run(null)}
          onDismiss={() => {
            markSetupDone();
            setGuideDismissed(true);
          }}
        />
      )}

      {plan?.warnings.map((w) => (
        <div key={w} className="banner warn">
          <TriangleAlert size={16} />
          {w}
        </div>
      ))}

      {/* ---------- 执行结果 ---------- */}
      {result && <ResultCard result={result} undone={undone} onUndo={() => setUndoFor(result.batch_id)} onContinue={() => run(null)} />}

      {!plan && !result && busy === "scan" && (
        <div className="empty muted">
          <Spinner size={22} />
          {t("正在扫描…")}
        </div>
      )}

      {/* ---------- 整理建议 ---------- */}
      {plan && (
        <>
          <div className="summary-line">
            <span>{t("{0} 个文件待整理", [items.length])}</span>
            {noMatch.length > 0 && (
              <button className="link" onClick={() => document.getElementById("no-match")?.scrollIntoView({ behavior: "smooth" })}>
                {t("{0} 个没有匹配的规则", [noMatch.length])}
              </button>
            )}
            {kept.length + attention.length > 0 && (
              <button
                className="link"
                onClick={() => {
                  setShowKept(true);
                  setTimeout(() => document.getElementById("kept")?.scrollIntoView({ behavior: "smooth" }), 0);
                }}
              >
                {t("{0} 项保持原位", [kept.length + attention.length])}
              </button>
            )}
          </div>

          {attention.length > 0 && (
            <div className="card">
              <div className="card-head">
                <TriangleAlert size={16} color="var(--warn)" />
                <h2>{t("需要留意")}</h2>
                <span className="muted">{t("这些文件本次不会移动")}</span>
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
                onChange={(on) =>
                  toggle(
                    items.map((i) => i.group),
                    on,
                  )
                }
                label={t("全选")}
              />
              <span className="text-2">{t("全选")}</span>
              <span className="spacer" />
              <Segmented
                value={groupBy}
                onChange={setGroupBy}
                options={[
                  { value: "target", label: t("按目标位置") },
                  { value: "rule", label: t("按整理原因") },
                ]}
              />
            </div>
          )}

          <ItemSections sections={sections} selected={selected} groupBy={groupBy} onToggle={toggle} onMenu={itemMenu} />

          {noMatch.length > 0 && (
            <NoMatchCard
              groups={noMatchByExtension}
              expanded={expandedExtensions}
              onToggle={(extension) =>
                setExpandedExtensions((prev) => {
                  const next = new Set(prev);
                  if (next.has(extension)) next.delete(extension);
                  else next.add(extension);
                  return next;
                })
              }
              onNewRule={() => onNewRule()}
              onMenu={skipMenu}
            />
          )}

          {kept.length > 0 && <KeptCard groups={keptByLabel} open={showKept} onToggle={() => setShowKept(!showKept)} onMenu={skipMenu} />}

          {items.length === 0 && noMatch.length === 0 && (config.rules ?? []).length > 0 && (
            <div className="empty">
              <div className="art">
                <CircleCheck size={26} />
              </div>
              <h2>{t("已经很整齐了")}</h2>
              <p className="muted">{t("没有需要整理的文件。")}</p>
            </div>
          )}

          {items.length > 0 && (
            <div className="footer-bar">
              <span className="text-2">{t("已选 {0} / {1} 个文件", [selected.size, items.length])}</span>
              <span className="spacer" />
              {confirmTrash && (
                <span className="chip warn" style={{ height: 30 }}>
                  <TriangleAlert size={14} />
                  {t("选中的文件里有要移到回收站的，确定继续？")}
                </span>
              )}
              {confirmTrash && (
                <button className="btn" onClick={() => setConfirmTrash(false)}>
                  {t("取消")}
                </button>
              )}
              <button
                data-testid="execute"
                className={`btn lg ${confirmTrash ? "danger" : "primary"}`}
                disabled={selected.size === 0 || busy !== null}
                onClick={execute}
              >
                {busy === "exec" ? <Spinner size={15} /> : <MousePointerClick size={16} />}
                {confirmTrash ? t("确定执行") : t("整理选中的 {0} 个文件", [selected.size])}
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
