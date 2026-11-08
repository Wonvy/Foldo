import {
  ArrowDownToLine,
  ArrowRight,
  ArrowUpToLine,
  ChevronLeft,
  ChevronRight,
  CircleCheck,
  Copy,
  Ellipsis,
  FileDown,
  FileUp,
  FileSearch,
  Folder,
  GripVertical,
  Layers,
  Link2,
  Lock,
  Package,
  PackageOpen,
  PenLine,
  Pencil,
  Plus,
  Search,
  ShieldCheck,
  Trash,
  TriangleAlert,
  WandSparkles,
  X,
  type LucideIcon,
} from "lucide-react";
import { useCallback, useEffect, useMemo, useRef, useState, type PointerEvent as ReactPointerEvent } from "react";
import { api, baseName, errorText, type DebugView, type RuleDef } from "../api";
import { GroupIcon } from "../components/FieldPicker";
import InspectDialog from "../components/InspectDialog";
import RuleEditor from "../components/RuleEditor";
import { FileIcon, Spinner, Switch, useMenu } from "../components/ui";
import { describeConditions, foldersUsing, ruleChips, tilde, type ActionKind } from "../rules";
import { pickFile, pickRulesFile, pickSavePath, useStore } from "../store";
import { t } from "../i18n";

const ACTION_ICON: Record<ActionKind, LucideIcon> = {
  move: ArrowRight,
  copy: Copy,
  rename: PenLine,
  extract: PackageOpen,
  compress: Package,
  shortcut: Link2,
  trash: Trash,
  keep: Lock,
};

const MAX_CHIPS = 3;

/** 其他页面请求打开规则编辑：编辑某条规则，或新建（可指定保存后加入的文件夹）。 */
export type RuleRequest = { n: number; edit?: string; assignTo?: string };

export default function RulesPage({
  request,
  dropped,
  onOpenAdvanced,
}: {
  /** 其他页面请求“新建规则”时变化。 */
  request: RuleRequest | null;
  /** 拖到规则页的文件：编辑面板打开时作为测试文件，否则用来调试规则。 */
  dropped: { paths: string[]; n: number } | null;
  onOpenAdvanced: () => void;
}) {
  const { config, save, view, fieldMap } = useStore();
  const rules = config.rules ?? [];
  const [error, setError] = useState<string | null>(null);
  const [query, setQuery] = useState("");
  // undefined：没打开；null：新建；数字：编辑第几条
  const [editing, setEditing] = useState<number | null | undefined>(undefined);
  const [testDrop, setTestDrop] = useState<{ path: string; n: number } | null>(null);
  // 新建的规则保存后加入这个文件夹
  const [assignTo, setAssignTo] = useState<string | undefined>(undefined);
  const menu = useMenu();

  // 调试
  const [debugPaths, setDebugPaths] = useState<string[]>([]);
  const [debugAt, setDebugAt] = useState(0);
  const [debug, setDebug] = useState<DebugView | null>(null);
  const [debugError, setDebugError] = useState<string | null>(null);
  const [debugging, setDebugging] = useState(false);
  const [inspect, setInspect] = useState<string | null>(null);

  useEffect(() => {
    if (!request) return;
    if (request.edit !== undefined) {
      const i = rules.findIndex((r) => r.name === request.edit);
      if (i >= 0) setEditing(i);
    } else {
      setAssignTo(request.assignTo);
      setEditing(null);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [request]);

  useEffect(() => {
    if (!dropped || dropped.paths.length === 0) return;
    if (editing !== undefined) {
      setTestDrop({ path: dropped.paths[0], n: dropped.n });
    } else {
      setDebugPaths(dropped.paths);
      setDebugAt(0);
    }
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [dropped]);

  const runDebug = useCallback(async (path: string) => {
    setDebugging(true);
    setDebugError(null);
    try {
      setDebug(await api.debugFile(path));
    } catch (e) {
      setDebug(null);
      setDebugError(errorText(e));
    } finally {
      setDebugging(false);
    }
  }, []);

  // 规则变化或切换文件时重新调试
  const current = debugPaths[debugAt];
  useEffect(() => {
    if (current) void runDebug(current);
    else setDebug(null);
  }, [current, config, runDebug]);

  const [notice, setNotice] = useState<string | null>(null);

  const exportRules = async (names: string[]) => {
    setError(null);
    const file = names.length === 1 ? `${names[0]}.yaml` : t("Foldo 规则.yaml");
    const path = await pickSavePath(t("导出规则"), file);
    if (!path) return;
    try {
      const n = await api.exportRules(names, path);
      setNotice(t("已导出 {0} 条规则", [n]));
    } catch (e) {
      setError(errorText(e));
    }
  };

  const importRules = async () => {
    setError(null);
    const path = await pickRulesFile(t("导入规则"));
    if (!path) return;
    try {
      const incoming = await api.readRulesFile(path);
      const names = new Set(rules.map((r) => r.name));
      const added = incoming.map((r) => {
        let name = r.name;
        for (let n = 1; names.has(name); n++) name = n === 1 ? t("{0}（导入）", [r.name]) : t("{0}（导入 {1}）", [r.name, n]);
        names.add(name);
        return { ...r, name };
      });
      await save({ ...config, rules: [...rules, ...added] });
      setNotice(t("已导入 {0} 条规则。在整理页把它们添加到文件夹后才会生效。", [added.length]));
    } catch (e) {
      setError(errorText(e));
    }
  };

  const update = async (next: RuleDef[]) => {
    setError(null);
    try {
      await save({ ...config, rules: next });
    } catch (e) {
      setError(errorText(e));
    }
  };

  const moveTo = (from: number, to: number) => {
    if (from === to) return;
    const next = [...rules];
    const [r] = next.splice(from, 1);
    next.splice(to, 0, r);
    void update(next);
  };

  const duplicate = (i: number) => {
    const names = new Set(rules.map((r) => r.name));
    let name = t("{0} 副本", [rules[i].name]);
    for (let n = 2; names.has(name); n++) name = t("{0} 副本 {1}", [rules[i].name, n]);
    const next = [...rules];
    next.splice(i + 1, 0, { ...rules[i], name });
    void update(next);
  };

  // ---------- 拖动排序（指针事件，不依赖 HTML5 拖放，Windows 上也可靠） ----------
  const rowRefs = useRef<(HTMLDivElement | null)[]>([]);
  const [drag, setDrag] = useState<{ from: number; over: number } | null>(null);
  const startDrag = (e: ReactPointerEvent, i: number) => {
    if (e.button !== 0) return;
    e.preventDefault();
    e.stopPropagation();
    setDrag({ from: i, over: i });
  };
  useEffect(() => {
    if (!drag) return;
    const move = (e: PointerEvent) => {
      let over = rules.length - 1;
      for (let i = 0; i < rowRefs.current.length; i++) {
        const r = rowRefs.current[i]?.getBoundingClientRect();
        if (r && e.clientY < r.top + r.height / 2) {
          over = i;
          break;
        }
      }
      // 往下拖时，落点是“越过的那一行”
      if (over > drag.from) over -= 1;
      if (e.clientY > (rowRefs.current[rules.length - 1]?.getBoundingClientRect().bottom ?? 0)) over = rules.length - 1;
      setDrag((d) => (d && d.over !== over ? { ...d, over } : d));
    };
    const up = () => {
      setDrag((d) => {
        if (d) moveTo(d.from, d.over);
        return null;
      });
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up, { once: true });
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [drag?.from, rules.length]);

  const shown = useMemo(() => {
    const q = query.trim().toLowerCase();
    return rules
      .map((r, i) => ({ r, i }))
      .filter(({ r }) => !q || `${r.name} ${describeConditions(r, fieldMap)}`.toLowerCase().includes(q));
  }, [rules, query, fieldMap]);
  const enabledCount = rules.filter((r) => r.enabled !== false).length;

  return (
    <div className="page rules-page">
      <div className="page-head">
        <div className="row" style={{ alignItems: "baseline", gap: 10 }}>
          <h1>{t("规则")}</h1>
          {rules.length > 0 && (
            <span className="muted">
              {enabledCount}/{rules.length} {t("启用")}
            </span>
          )}
        </div>
        <div className="row">
          {rules.length > 3 && (
            <div className="search-box rules-search">
              <Search size={14} />
              <input placeholder={t("搜索规则")} value={query} onChange={(e) => setQuery(e.target.value)} />
            </div>
          )}
          <button className="btn primary" onClick={() => setEditing(null)}>
            <Plus size={16} /> {t("新建规则")}
          </button>
          <button
            className="icon-btn"
            aria-label={t("更多")}
            onClick={(e) =>
              menu.open(e.currentTarget, [
                { label: t("导入规则…"), icon: <FileUp size={15} />, onClick: importRules },
                ...(rules.length
                  ? [{ label: t("导出全部规则…"), icon: <FileDown size={15} />, onClick: () => exportRules([]) }]
                  : []),
              ])
            }
          >
            <Ellipsis size={16} />
          </button>
        </div>
      </div>

      {notice && (
        <div className="banner info">
          <CircleCheck size={16} />
          <span style={{ flex: 1 }}>{notice}</span>
          <button className="icon-btn" aria-label={t("关闭")} onClick={() => setNotice(null)}>
            <X size={15} />
          </button>
        </div>
      )}

      {view?.error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          <span style={{ flex: 1 }}>{t("配置文件有错误：{0}", [view.error])}</span>
          <button className="link" onClick={onOpenAdvanced}>
            {t("去修正")}
          </button>
        </div>
      )}
      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          <span className="selectable">{error}</span>
        </div>
      )}

      {rules.length === 0 ? (
        <div className="empty" style={{ padding: "60px 16px" }}>
          <div className="art">
            <WandSparkles size={24} />
          </div>
          <h2>{t("还没有规则")}</h2>
          <p className="muted">{t("规则决定哪些文件放到哪里，从上到下判断，第一条命中的生效。")}</p>
          <button className="btn primary" style={{ marginTop: 8 }} onClick={() => setEditing(null)}>
            <Plus size={16} /> {t("新建规则")}
          </button>
        </div>
      ) : (
        <div className={`rule-list ${drag ? "dragging" : ""}`}>
          {shown.map(({ r, i }) => {
            const enabled = r.enabled !== false;
            const hit = debug?.rules[i];
            const winner = debug?.winner === i;
            const dim = debug && !winner && !(hit?.matched && enabled);
            const chips = ruleChips(r, fieldMap);
            const extra = chips.conds.length - MAX_CHIPS;
            const dropBefore = drag && drag.over === i && drag.over < drag.from;
            const dropAfter = drag && drag.over === i && drag.over > drag.from;
            return (
              <div
                key={r.name}
                ref={(el) => {
                  rowRefs.current[i] = el;
                }}
                className={[
                  "rule-card",
                  enabled ? "" : "off",
                  winner ? "winner" : "",
                  dim ? "dim" : "",
                  drag?.from === i ? "lifted" : "",
                  dropBefore ? "drop-before" : "",
                  dropAfter ? "drop-after" : "",
                ].join(" ")}
                onClick={() => !drag && setEditing(i)}
              >
                <span
                  className="grip"
                  title={query ? t("清空搜索后可拖动排序") : t("拖动调整列表及拖入文件的匹配顺序；文件夹优先级在整理页调整")}
                  onPointerDown={(e) => !query && startDrag(e, i)}
                  onClick={(e) => e.stopPropagation()}
                >
                  <GripVertical size={16} />
                </span>
                <span className="rule-no">{i + 1}</span>
                <div className="rule-main">
                  <div className="rule-name">
                    <span className="ellipsis">{r.name}</span>
                    {winner && (
                      <span className="chip ok">
                        <CircleCheck size={12} /> {t("处理")}
                      </span>
                    )}
                    {debug && !winner && hit?.matched && enabled && <span className="chip">{t("也满足")}</span>}
                    {debug && hit?.matched && !hit.in_scope && <span className="chip warn" title={t("这个文件所在的文件夹没有使用这条规则")}>{t("文件夹未使用")}</span>}
                  </div>
                  <div className="rule-chips">
                    {(() => {
                      const used = foldersUsing(config, r.name);
                      return used.length ? (
                        <span className="cchip scope" title={used.map((f) => tilde(f.path)).join("\n")}>
                          <Folder size={12} />
                          {used.map((f) => baseName(f.path)).join(t("、"))}
                        </span>
                      ) : (
                        <span className="cchip warn" title={t("在“文件夹”页把这条规则添加到文件夹后才会生效")}>
                          <Folder size={12} />
                          {t("未用于文件夹")}
                        </span>
                      );
                    })()}
                    {r.target === "folder" && (
                      <span className="cchip mode" title={t("这条规则处理文件夹第一层的子文件夹")}>
                        <Folder size={12} /> {t("文件夹")}
                      </span>
                    )}
                    {chips.mode !== "all" && chips.conds.length > 1 && <span className="cchip mode">{chips.mode === "any" ? t("任一") : t("都不")}</span>}
                    {chips.conds.length === 0 && <span className="cchip">{t("所有文件")}</span>}
                    {chips.conds.slice(0, MAX_CHIPS).map((c, k) => (
                      <span key={k} className={`cchip ${c.not ? "neg" : ""}`} title={c.title}>
                        {c.not && <span className="neg-mark">{t("非")}</span>}
                        {c.kind === "group" ? <Layers size={12} /> : <GroupIcon group={c.group} size={12} />}
                        <span className="ellipsis">{c.text}</span>
                      </span>
                    ))}
                    {extra > 0 && (
                      <span className="cchip" title={chips.conds.slice(MAX_CHIPS).map((c) => c.title).join("\n")}>
                        +{extra}
                      </span>
                    )}
                    {chips.exceptions > 0 && (
                      <span className="cchip neg" title={t("满足例外时不处理")}>
                        <ShieldCheck size={12} /> {t("{0} 个例外", [chips.exceptions])}
                      </span>
                    )}
                    <span className="flow-arrow">
                      <ArrowRight size={13} />
                    </span>
                    {chips.actions.map((a, k) => {
                      const Icon = ACTION_ICON[a.kind];
                      return (
                        <span key={k} className={`cchip act ${a.kind}`} title={a.title}>
                          <Icon size={12} />
                          <span className="ellipsis">{a.text}</span>
                        </span>
                      );
                    })}
                  </div>
                </div>
                <span className="rule-side" onClick={(e) => e.stopPropagation()}>
                  <Switch on={enabled} label={t("启用")} onChange={(on) => update(rules.map((x, j) => (j === i ? { ...x, enabled: on } : x)))} />
                  <button
                    className="icon-btn"
                    aria-label={t("更多")}
                    onClick={(e) =>
                      menu.open(e.currentTarget, [
                        { label: t("编辑"), icon: <Pencil size={15} />, onClick: () => setEditing(i) },
                        { label: t("复制一份"), icon: <Copy size={15} />, onClick: () => duplicate(i) },
                        { label: t("导出…"), icon: <FileDown size={15} />, onClick: () => exportRules([r.name]) },
                        ...(i > 0 ? [{ label: t("移到最前"), icon: <ArrowUpToLine size={15} />, onClick: () => moveTo(i, 0) }] : []),
                        ...(i < rules.length - 1
                          ? [{ label: t("移到最后"), icon: <ArrowDownToLine size={15} />, onClick: () => moveTo(i, rules.length - 1) }]
                          : []),
                        "divider" as const,
                        { label: t("删除规则"), icon: <Trash size={15} />, danger: true, onClick: () => update(rules.filter((_, j) => j !== i)) },
                      ])
                    }
                  >
                    <Ellipsis size={17} />
                  </button>
                </span>
              </div>
            );
          })}
          {shown.length === 0 && <div className="muted" style={{ padding: 20, textAlign: "center" }}>{t("没有匹配的规则")}</div>}
        </div>
      )}

      {/* 调试状态栏 */}
      <div className={`debug-bar ${debug ? (debug.winner !== null && !debug.excluded ? "ok" : "none") : ""}`}>
        {!current ? (
          <button
            className="debug-hint"
            onClick={async () => {
              const p = await pickFile(t("选择一个文件，看它会被哪条规则处理"));
              if (p) {
                setDebugPaths([p]);
                setDebugAt(0);
              }
            }}
          >
            <FileDown size={16} /> {t("拖入文件，看它会被哪条规则处理")}
          </button>
        ) : (
          <>
            {debugging && !debug ? <Spinner size={15} /> : <FileIcon category={debug?.category} size={26} />}
            <button className="debug-name ellipsis" title={t("{0}\n点击查看全部属性", [current])} onClick={() => setInspect(current)}>
              {debug?.name ?? current.split(/[\\/]/).pop()}
            </button>
            <span className="debug-result ellipsis">
              {debugError ? (
                <span style={{ color: "var(--danger)" }}>{debugError}</span>
              ) : debug?.excluded ? (
                <>
                  <ShieldCheck size={14} /> {t("不会被处理：{0}", [debug.excluded])}
                </>
              ) : debug && debug.winner !== null ? (
                <>
                  {t("第 {0} 条「{1}」· {2}", [debug.winner + 1, rules[debug.winner]?.name ?? "", debug.action ? t(debug.action) : t("处理")])}
                  {debug.destination && <span className="mono"> {tilde(debug.destination)}</span>}
                  {!debug.destination && debug.reason && t("：{0}", [debug.reason])}
                </>
              ) : debug ? (
                <>{t("没有规则命中，保持原位")}</>
              ) : null}
            </span>
            <span className="spacer" />
            {debugPaths.length > 1 && (
              <span className="row" style={{ gap: 2 }}>
                <button className="icon-btn" disabled={debugAt === 0} onClick={() => setDebugAt(debugAt - 1)} aria-label={t("上一个")}>
                  <ChevronLeft size={16} />
                </button>
                <span className="muted mono" style={{ fontSize: 12 }}>
                  {debugAt + 1}/{debugPaths.length}
                </span>
                <button className="icon-btn" disabled={debugAt === debugPaths.length - 1} onClick={() => setDebugAt(debugAt + 1)} aria-label={t("下一个")}>
                  <ChevronRight size={16} />
                </button>
              </span>
            )}
            <button className="icon-btn" title={t("查看文件属性")} onClick={() => setInspect(current)}>
              <FileSearch size={16} />
            </button>
            <button className="icon-btn" title={t("结束调试")} onClick={() => (setDebugPaths([]), setDebug(null))}>
              <X size={16} />
            </button>
          </>
        )}
      </div>

      {menu.node}
      {inspect && <InspectDialog path={inspect} onClose={() => setInspect(null)} />}
      {editing !== undefined && (
        <RuleEditor
          index={editing}
          testDrop={testDrop}
          assignTo={editing === null ? assignTo : undefined}
          onClose={() => {
            setEditing(undefined);
            setTestDrop(null);
            setAssignTo(undefined);
          }}
          onOpenAdvanced={onOpenAdvanced}
        />
      )}
    </div>
  );
}
