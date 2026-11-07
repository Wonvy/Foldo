import {
  ArrowDown,
  ArrowUp,
  ChevronLeft,
  ChevronRight,
  CircleCheck,
  Copy,
  Ellipsis,
  FileDown,
  FileSearch,
  Pencil,
  Plus,
  ShieldCheck,
  Trash,
  TriangleAlert,
  WandSparkles,
  X,
} from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { api, errorText, type DebugView, type RuleDef } from "../api";
import InspectDialog from "../components/InspectDialog";
import RuleEditor from "../components/RuleEditor";
import { FileIcon, Spinner, Switch, useMenu } from "../components/ui";
import { describeActions, describeConditions, describeExceptions, describeScope, tilde } from "../rules";
import { pickFile, useStore } from "../store";

export default function RulesPage({
  newRequest,
  dropped,
  onOpenAdvanced,
}: {
  /** 其他页面请求“新建规则”时变化。 */
  newRequest: number;
  /** 拖到规则页的文件：编辑面板打开时作为测试文件，否则用来调试规则。 */
  dropped: { paths: string[]; n: number } | null;
  onOpenAdvanced: () => void;
}) {
  const { config, save, view, fieldMap } = useStore();
  const rules = config.rules ?? [];
  const [error, setError] = useState<string | null>(null);
  // undefined：没打开；null：新建；数字：编辑第几条
  const [editing, setEditing] = useState<number | null | undefined>(undefined);
  const [testDrop, setTestDrop] = useState<{ path: string; n: number } | null>(null);
  const menu = useMenu();

  // 调试
  const [debugPaths, setDebugPaths] = useState<string[]>([]);
  const [debugAt, setDebugAt] = useState(0);
  const [debug, setDebug] = useState<DebugView | null>(null);
  const [debugError, setDebugError] = useState<string | null>(null);
  const [debugging, setDebugging] = useState(false);
  const [inspect, setInspect] = useState<string | null>(null);

  useEffect(() => {
    if (newRequest > 0) setEditing(null);
  }, [newRequest]);

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

  const update = async (next: RuleDef[]) => {
    setError(null);
    try {
      await save({ ...config, rules: next });
    } catch (e) {
      setError(errorText(e));
    }
  };

  const move = (i: number, d: -1 | 1) => {
    const next = [...rules];
    [next[i], next[i + d]] = [next[i + d], next[i]];
    void update(next);
  };

  const duplicate = (i: number) => {
    const names = new Set(rules.map((r) => r.name));
    let name = `${rules[i].name} 副本`;
    for (let n = 2; names.has(name); n++) name = `${rules[i].name} 副本 ${n}`;
    const next = [...rules];
    next.splice(i + 1, 0, { ...rules[i], name });
    void update(next);
  };

  const hitOf = (i: number) => debug?.rules[i];

  return (
    <div className="page rules-page">
      <div className="page-head">
        <div>
          <h1>规则</h1>
          <p className="sub">从上到下判断，第一条命中的生效。把文件拖进来可以调试。</p>
        </div>
        <button className="btn primary" onClick={() => setEditing(null)}>
          <Plus size={16} /> 新建规则
        </button>
      </div>

      {view?.error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          <span style={{ flex: 1 }}>配置文件有错误：{view.error}</span>
          <button className="link" onClick={onOpenAdvanced}>
            去修正
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
        <div className="card">
          <div className="empty" style={{ padding: "40px 16px" }}>
            <div className="art">
              <WandSparkles size={24} />
            </div>
            <h2>还没有规则</h2>
            <button className="btn primary" style={{ marginTop: 8 }} onClick={() => setEditing(null)}>
              <Plus size={16} /> 新建规则
            </button>
          </div>
        </div>
      ) : (
        <div className="card">
          {rules.map((r, i) => {
            const ex = describeExceptions(r, fieldMap);
            const enabled = r.enabled !== false;
            const hit = hitOf(i);
            const winner = debug?.winner === i;
            const dim = debug && !winner && !(hit?.matched && enabled);
            return (
              <div
                key={r.name}
                className={`list-row clickable rule-row ${enabled ? "" : "off"} ${winner ? "winner" : ""} ${dim ? "dim" : ""}`}
                onClick={() => setEditing(i)}
              >
                <span className="rule-index mono">{i + 1}</span>
                <div className="row-main" style={{ gap: 4 }}>
                  <div className="row-title row">
                    {r.name}
                    {winner && (
                      <span className="chip ok">
                        <CircleCheck size={12} /> 处理
                      </span>
                    )}
                    {debug && !winner && hit?.matched && enabled && <span className="chip">也满足</span>}
                    {debug && hit?.matched && !hit.in_scope && <span className="chip warn">不在生效范围</span>}
                  </div>
                  <div className="rule-sentence">
                    在 <b>{describeScope(r)}</b> 中，{describeConditions(r, fieldMap)}
                    {ex && `（${ex}除外）`} → <b>{describeActions(r)}</b>
                  </div>
                </div>
                <span onClick={(e) => e.stopPropagation()} className="row">
                  <Switch
                    on={enabled}
                    label="启用"
                    onChange={(on) => update(rules.map((x, j) => (j === i ? { ...x, enabled: on } : x)))}
                  />
                  <button className="icon-btn" disabled={i === 0} onClick={() => move(i, -1)} aria-label="提高优先级" title="提高优先级">
                    <ArrowUp size={16} />
                  </button>
                  <button className="icon-btn" disabled={i === rules.length - 1} onClick={() => move(i, 1)} aria-label="降低优先级" title="降低优先级">
                    <ArrowDown size={16} />
                  </button>
                  <button
                    className="icon-btn"
                    aria-label="更多"
                    onClick={(e) =>
                      menu.open(e.currentTarget, [
                        { label: "编辑", icon: <Pencil size={15} />, onClick: () => setEditing(i) },
                        { label: "复制一份", icon: <Copy size={15} />, onClick: () => duplicate(i) },
                        "divider",
                        { label: "删除规则", icon: <Trash size={15} />, danger: true, onClick: () => update(rules.filter((_, j) => j !== i)) },
                      ])
                    }
                  >
                    <Ellipsis size={17} />
                  </button>
                </span>
              </div>
            );
          })}
        </div>
      )}

      {/* 调试状态栏 */}
      <div className={`debug-bar ${debug ? (debug.winner !== null && !debug.excluded ? "ok" : "none") : ""}`}>
        {!current ? (
          <button
            className="debug-hint"
            onClick={async () => {
              const p = await pickFile("选择一个文件，看它会被哪条规则处理");
              if (p) {
                setDebugPaths([p]);
                setDebugAt(0);
              }
            }}
          >
            <FileDown size={16} /> 把文件拖到这里，看它会被哪条规则处理
          </button>
        ) : (
          <>
            {debugging && !debug ? <Spinner size={15} /> : <FileIcon category={debug?.category} size={26} />}
            <button className="debug-name ellipsis" title={`${current}\n点击查看全部属性`} onClick={() => setInspect(current)}>
              {debug?.name ?? current.split(/[\\/]/).pop()}
            </button>
            <span className="debug-result ellipsis">
              {debugError ? (
                <span style={{ color: "var(--danger)" }}>{debugError}</span>
              ) : debug?.excluded ? (
                <>
                  <ShieldCheck size={14} /> 不会被处理：{debug.excluded}
                </>
              ) : debug && debug.winner !== null ? (
                <>
                  第 {debug.winner + 1} 条「{rules[debug.winner]?.name}」· {debug.action ?? "处理"}
                  {debug.destination && <span className="mono"> {tilde(debug.destination)}</span>}
                  {!debug.destination && debug.reason && `：${debug.reason}`}
                </>
              ) : debug ? (
                <>没有规则命中，保持原位</>
              ) : null}
            </span>
            <span className="spacer" />
            {debugPaths.length > 1 && (
              <span className="row" style={{ gap: 2 }}>
                <button className="icon-btn" disabled={debugAt === 0} onClick={() => setDebugAt(debugAt - 1)} aria-label="上一个">
                  <ChevronLeft size={16} />
                </button>
                <span className="muted mono" style={{ fontSize: 12 }}>
                  {debugAt + 1}/{debugPaths.length}
                </span>
                <button className="icon-btn" disabled={debugAt === debugPaths.length - 1} onClick={() => setDebugAt(debugAt + 1)} aria-label="下一个">
                  <ChevronRight size={16} />
                </button>
              </span>
            )}
            <button className="icon-btn" title="查看文件属性" onClick={() => setInspect(current)}>
              <FileSearch size={16} />
            </button>
            <button className="icon-btn" title="结束调试" onClick={() => (setDebugPaths([]), setDebug(null))}>
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
          onClose={() => {
            setEditing(undefined);
            setTestDrop(null);
          }}
          onOpenAdvanced={onOpenAdvanced}
        />
      )}
    </div>
  );
}
