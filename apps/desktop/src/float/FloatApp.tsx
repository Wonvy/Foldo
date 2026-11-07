// 悬浮窗（类似 DropIt）：置顶的小图标，把文件拖上去就按规则给出整理方案，确认后执行，可撤销。
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppWindow, ArrowRight, CircleCheck, FolderTree, RotateCcw, Trash, TriangleAlert, Upload, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { api, baseName, errorText, type ExecView, type PlanView } from "../api";
import { FileIcon, Spinner } from "../components/ui";

type Phase = "idle" | "busy" | "plan" | "done";

const COLLAPSED: [number, number] = [84, 84];
const EXPANDED: [number, number] = [340, 380];

export default function FloatApp() {
  const [hover, setHover] = useState(false);
  const [phase, setPhase] = useState<Phase>("idle");
  const [plan, setPlan] = useState<PlanView | null>(null);
  const [result, setResult] = useState<ExecView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [undone, setUndone] = useState(false);
  const [running, setRunning] = useState(false);
  const collapseTimer = useRef<ReturnType<typeof setTimeout>>(undefined);

  const expanded = phase !== "idle";
  useEffect(() => {
    void api.floatResize(...(expanded ? EXPANDED : COLLAPSED)).catch(() => {});
  }, [expanded]);

  const handle = async (paths: string[]) => {
    clearTimeout(collapseTimer.current);
    setPhase("busy");
    setError(null);
    setResult(null);
    setUndone(false);
    try {
      setPlan(await api.scanPaths(paths));
      setPhase("plan");
    } catch (e) {
      setError(errorText(e));
      setPhase("plan");
    }
  };

  // 拖放
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        const un = await getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") setHover(true);
          else if (p.type === "leave") setHover(false);
          else if (p.type === "drop") {
            setHover(false);
            if (p.paths.length) void handle(p.paths);
          }
        });
        if (disposed) un();
        else unlisten = un;
      } catch {
        /* 浏览器预览 */
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const items = useMemo(() => {
    if (!plan) return [];
    const byGroup = new Map<number, (typeof plan.ops)[number][]>();
    for (const o of plan.ops) byGroup.set(o.group, [...(byGroup.get(o.group) ?? []), o]);
    return [...byGroup.values()].map((ops) => {
      const last = ops[ops.length - 1];
      return { group: last.group, name: last.origin_name, category: last.category, kind: last.kind, dir: last.dst_dir_display, rule: last.rule };
    });
  }, [plan]);
  const skipped = plan?.skipped ?? [];

  const execute = async () => {
    if (!plan) return;
    setRunning(true);
    try {
      const r = await api.executePlan(plan.plan_id, items.map((i) => i.group));
      setResult(r);
      setPhase("done");
      collapseTimer.current = setTimeout(() => setPhase("idle"), 10000);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setRunning(false);
    }
  };

  const undo = async () => {
    if (!result) return;
    clearTimeout(collapseTimer.current);
    try {
      await api.undo(result.batch_id);
      setUndone(true);
      collapseTimer.current = setTimeout(() => setPhase("idle"), 4000);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const drag = (e: MouseEvent) => {
    if (e.button === 0 && e.detail === 1) void getCurrentWindow().startDragging().catch(() => {});
  };

  const target = (small: boolean) => (
    <div
      className={`float-target ${small ? "small" : ""} ${hover ? "hover" : ""}`}
      onMouseDown={drag}
      onDoubleClick={() => void api.showMain()}
      onContextMenu={(e) => {
        e.preventDefault();
        void api.toggleFloat();
      }}
      title="把文件拖到这里整理 · 双击打开主窗口 · 右键关闭"
    >
      {hover ? <Upload size={small ? 16 : 26} /> : <FolderTree size={small ? 16 : 26} />}
    </div>
  );

  if (!expanded) return <div className="float-root">{target(false)}</div>;

  return (
    <div className={`float-panel ${hover ? "hover" : ""}`}>
      <div className="float-head" onMouseDown={drag}>
        {target(true)}
        <strong className="ellipsis">
          {phase === "busy"
            ? "正在分析…"
            : phase === "done"
              ? undone
                ? "已撤销"
                : `已整理 ${result?.done ?? 0} 项`
              : `${items.length} 个文件可以整理`}
        </strong>
        <span className="spacer" />
        <button className="icon-btn" title="打开主窗口" onClick={() => void api.showMain()}>
          <AppWindow size={15} />
        </button>
        <button className="icon-btn" title="收起" onClick={() => setPhase("idle")}>
          <X size={15} />
        </button>
      </div>

      <div className="float-body">
        {phase === "busy" && (
          <div className="float-center muted">
            <Spinner size={20} />
          </div>
        )}
        {error && (
          <div className="banner error">
            <TriangleAlert size={15} />
            <span>{error}</span>
          </div>
        )}
        {phase === "plan" &&
          items.map((it) => (
            <div key={it.group} className="float-item" title={`规则「${it.rule}」`}>
              <FileIcon category={it.category} size={26} />
              <span className="ellipsis name">{it.name}</span>
              <span className="float-to ellipsis">
                {it.kind === "trash" ? (
                  <>
                    <Trash size={12} /> 回收站
                  </>
                ) : (
                  <>
                    <ArrowRight size={12} /> {baseName(it.dir ?? "")}
                  </>
                )}
              </span>
            </div>
          ))}
        {phase === "plan" &&
          skipped.map((s) => (
            <div key={s.path} className="float-item off" title={s.reason}>
              <FileIcon category={null} size={26} />
              <span className="ellipsis name">{s.name}</span>
              <span className="float-to ellipsis">{s.label}</span>
            </div>
          ))}
        {phase === "done" && result && (
          <div className="float-center">
            <CircleCheck size={30} color="var(--ok)" />
            <div className="muted" style={{ fontSize: 12.5 }}>
              {undone
                ? "文件已放回原处"
                : [result.skipped.length && `跳过 ${result.skipped.length} 项`, result.failed.length && `失败 ${result.failed.length} 项`].filter(Boolean).join("，") ||
                  "全部完成"}
            </div>
          </div>
        )}
      </div>

      <div className="float-foot">
        {phase === "plan" && (
          <button className="btn primary" style={{ flex: 1 }} disabled={items.length === 0 || running} onClick={execute}>
            {running ? <Spinner size={14} /> : null}
            {items.length ? `整理 ${items.length} 个文件` : "没有可整理的文件"}
          </button>
        )}
        {phase === "done" && !undone && result && result.done > 0 && (
          <button className="btn" style={{ flex: 1 }} onClick={undo}>
            <RotateCcw size={14} /> 撤销
          </button>
        )}
      </div>
    </div>
  );
}
