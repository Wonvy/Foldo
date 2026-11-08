// 悬浮窗（类似 DropIt）：置顶的小图标，把文件拖上去就按规则给出整理方案，确认后执行，可撤销。
import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { AppWindow, ArrowRight, CircleCheck, FolderTree, RotateCcw, Trash, TriangleAlert, Upload, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { api, baseName, errorText, type ExecView, type PlanView } from "../api";
import { FileIcon, Spinner } from "../components/ui";
import { t } from "../i18n";

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
  const [onTop, setOnTop] = useState(true);
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
      setPhase("done");
      collapseTimer.current = setTimeout(() => setPhase("idle"), 4000);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const pickAndHandle = async () => {
    try {
      const { open } = await import("@tauri-apps/plugin-dialog");
      const r = await open({ multiple: true, directory: false, title: t("选择要整理的文件") });
      const paths = Array.isArray(r) ? r : typeof r === "string" ? [r] : [];
      if (paths.length) await handle(paths);
    } catch (e) {
      setError(errorText(e));
    }
  };

  // 右键菜单（系统原生菜单，收起时窗口很小也能完整显示）
  const openMenu = async (e: MouseEvent) => {
    e.preventDefault();
    const canUndo = !!result && result.done > 0 && !undone;
    try {
      const sep = () => PredefinedMenuItem.new({ item: "Separator" });
      const menu = await Menu.new({
        items: [
          await MenuItem.new({ text: t("打开主窗口"), action: () => void api.showMain() }),
          await MenuItem.new({ text: t("选择文件整理…"), action: () => void pickAndHandle() }),
          await MenuItem.new({ text: t("撤销上次整理"), enabled: canUndo, action: () => void undo() }),
          await sep(),
          await MenuItem.new({ text: expanded ? t("收起面板") : t("展开上次结果"), enabled: expanded || !!plan, action: () => setPhase(expanded ? "idle" : result ? "done" : "plan") }),
          await CheckMenuItem.new({
            text: t("始终置顶"),
            checked: onTop,
            action: () => {
              const v = !onTop;
              void getCurrentWindow()
                .setAlwaysOnTop(v)
                .then(() => setOnTop(v))
                .catch(() => {});
            },
          }),
          await sep(),
          await MenuItem.new({ text: t("关闭悬浮窗"), action: () => void api.toggleFloat() }),
        ],
      });
      await menu.popup();
    } catch {
      /* 浏览器预览中没有原生菜单 */
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
      onContextMenu={openMenu}
      title={t("把文件拖到这里整理 · 双击打开主窗口 · 右键更多操作")}
    >
      {hover ? <Upload size={small ? 16 : 26} /> : <FolderTree size={small ? 16 : 26} />}
    </div>
  );

  if (!expanded) return <div className="float-root">{target(false)}</div>;

  return (
    <div className={`float-panel ${hover ? "hover" : ""}`} onContextMenu={openMenu}>
      <div className="float-head" onMouseDown={drag}>
        {target(true)}
        <strong className="ellipsis">
          {phase === "busy"
            ? t("正在分析…")
            : phase === "done"
              ? undone
                ? t("已撤销")
                : t("已整理 {0} 项", [result?.done ?? 0])
              : t("{0} 个文件可以整理", [items.length])}
        </strong>
        <span className="spacer" />
        <button className="icon-btn" title={t("打开主窗口")} onClick={() => void api.showMain()}>
          <AppWindow size={15} />
        </button>
        <button className="icon-btn" title={t("收起")} onClick={() => setPhase("idle")}>
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
            <div key={it.group} className="float-item" title={t("规则「{0}」", [it.rule])}>
              <FileIcon category={it.category} size={26} />
              <span className="ellipsis name">{it.name}</span>
              <span className="float-to ellipsis">
                {it.kind === "trash" ? (
                  <>
                    <Trash size={12} /> {t("回收站")}
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
                ? t("文件已放回原处")
                : [result.skipped.length && t("跳过 {0} 项", [result.skipped.length]), result.failed.length && t("失败 {0} 项", [result.failed.length])].filter(Boolean).join(t("，")) ||
                  t("全部完成")}
            </div>
          </div>
        )}
      </div>

      <div className="float-foot">
        {phase === "plan" && (
          <button className="btn primary" style={{ flex: 1 }} disabled={items.length === 0 || running} onClick={execute}>
            {running ? <Spinner size={14} /> : null}
            {items.length ? t("整理 {0} 个文件", [items.length]) : t("没有可整理的文件")}
          </button>
        )}
        {phase === "done" && !undone && result && result.done > 0 && (
          <button className="btn" style={{ flex: 1 }} onClick={undo}>
            <RotateCcw size={14} /> {t("撤销")}
          </button>
        )}
      </div>
    </div>
  );
}
