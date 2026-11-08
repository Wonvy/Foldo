// 悬浮窗（类似 DropIt）：置顶的小图标，把文件拖上去就按规则给出整理方案，确认后执行，可撤销。
import { CheckMenuItem, Menu, MenuItem, PredefinedMenuItem } from "@tauri-apps/api/menu";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { emitTo, listen } from "@tauri-apps/api/event";
import { AppWindow, CircleCheck, FolderTree, ListX, Minus, Plus, RotateCcw, TriangleAlert, Upload, X } from "lucide-react";
import { useEffect, useMemo, useRef, useState, type MouseEvent } from "react";
import { api, errorText, type ExecView, type PlanView } from "../api";
import { Spinner } from "../components/ui";
import { t } from "../i18n";
import FileList from "./FileList";
import { loadView, mergePaths, pathKey, toItems, type View } from "./items";

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
  // 拖入后取消勾选的（不整理）和从列表里移除的
  const [unchecked, setUnchecked] = useState<Set<string>>(new Set());
  const [removed, setRemoved] = useState<Set<string>>(new Set());
  const [view, setView] = useState<View>(loadView);
  const collapseTimer = useRef<ReturnType<typeof setTimeout>>(undefined);
  const pathsRef = useRef<string[]>([]);
  const scanId = useRef(0);
  const phaseRef = useRef(phase);
  phaseRef.current = phase;
  const runningRef = useRef(running);
  runningRef.current = running;

  const expanded = phase !== "idle";
  useEffect(() => {
    void api.floatResize(...(expanded ? EXPANDED : COLLAPSED)).catch(() => {});
  }, [expanded]);

  const scan = async (paths: string[]) => {
    const request = ++scanId.current;
    clearTimeout(collapseTimer.current);
    phaseRef.current = "busy";
    setPhase("busy");
    setPlan(null);
    setError(null);
    setResult(null);
    setUndone(false);
    try {
      const next = await api.scanPaths(paths);
      if (request !== scanId.current) return;
      setPlan(next);
      setPhase("plan");
    } catch (e) {
      if (request !== scanId.current) return;
      setError(errorText(e));
      setPhase("plan");
    }
  };

  const handle = async (paths: string[]) => {
    if (!paths.length || runningRef.current) return;
    if (phaseRef.current === "done" || result) {
      pathsRef.current = [];
      setUnchecked(new Set());
      setRemoved(new Set());
    }
    pathsRef.current = mergePaths(pathsRef.current, paths);
    // Explicitly dropping a removed file adds it back.
    setRemoved((previous) => new Set([...previous].filter((key) => !paths.some((path) => pathKey(path) === key))));
    await scan(pathsRef.current);
  };
  const handleRef = useRef(handle);
  handleRef.current = handle;
  const rescanRef = useRef(() => {});
  rescanRef.current = () => {
    if (pathsRef.current.length && !runningRef.current && phaseRef.current !== "done") void scan(pathsRef.current);
  };

  const clear = () => {
    if (runningRef.current) return;
    ++scanId.current;
    clearTimeout(collapseTimer.current);
    pathsRef.current = [];
    setPlan(null);
    setResult(null);
    setUnchecked(new Set());
    setRemoved(new Set());
    setError(null);
    setPhase("plan");
  };

  const changeView = (next: View) => {
    setView(next);
    try {
      localStorage.setItem("foldo.float.view", next);
    } catch {
      /* Storage is optional. */
    }
  };

  // 拖放
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const un = await getCurrentWindow().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") setHover(true);
          else if (p.type === "leave") setHover(false);
          else if (p.type === "drop") {
            setHover(false);
            if (p.paths.length) void handleRef.current(p.paths);
          }
        });
        if (disposed) un();
        else unlisten = un;
      } catch (e) {
        if (!disposed) setError(t("无法接收拖入文件，请使用添加文件按钮：{0}", [errorText(e)]));
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
      ++scanId.current;
      clearTimeout(collapseTimer.current);
    };
  }, []);

  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    void listen("config-changed", () => rescanRef.current())
      .then((un) => (disposed ? un() : (unlisten = un)))
      .catch(() => {});
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  const items = useMemo(() => toItems(plan).filter((item) => !removed.has(pathKey(item.path))), [plan, removed]);
  const skipped = (plan?.skipped ?? []).filter((s) => !removed.has(pathKey(s.path)));
  const chosen = items.filter((i) => !unchecked.has(pathKey(i.path)));

  const toggle = (paths: string[], on: boolean) =>
    setUnchecked((s) => {
      const n = new Set(s);
      for (const path of paths)
        if (on) n.delete(pathKey(path));
        else n.add(pathKey(path));
      return n;
    });
  const remove = (paths: string[]) => {
    const keys = new Set(paths.map(pathKey));
    pathsRef.current = pathsRef.current.filter((path) => !keys.has(pathKey(path)));
    setRemoved((s) => new Set([...s, ...keys]));
  };

  const newRule = async (path: string) => {
    try {
      await api.showMain();
      await emitTo("main", "float-new-rule", { path });
    } catch (e) {
      setError(errorText(e));
    }
  };

  const close = () => {
    void api.toggleFloat().catch((e) => setError(errorText(e)));
  };

  // 条目的右键菜单
  const itemMenu = async (e: MouseEvent, path: string, group?: number, noMatch?: boolean) => {
    e.preventDefault();
    e.stopPropagation();
    try {
      const entries = [];
      if (group !== undefined)
        entries.push(
          await CheckMenuItem.new({
            text: t("整理这个文件"),
            checked: !unchecked.has(pathKey(path)),
            action: () => toggle([path], unchecked.has(pathKey(path))),
          }),
        );
      if (noMatch) entries.push(await MenuItem.new({ text: t("添加规则"), action: () => void newRule(path) }));
      entries.push(
        await MenuItem.new({ text: t("从列表中移除"), action: () => remove([path]) }),
        await MenuItem.new({ text: t("在文件管理器中显示"), action: () => void api.reveal(path) }),
      );
      await (await Menu.new({ items: entries })).popup();
    } catch {
      /* 浏览器预览中没有原生菜单 */
    }
  };
  const execute = async () => {
    if (!plan || !chosen.length || runningRef.current || phase !== "plan") return;
    runningRef.current = true;
    setRunning(true);
    try {
      const r = await api.executePlan(
        plan.plan_id,
        chosen.map((i) => i.group),
      );
      setResult(r);
      setPhase("done");
      collapseTimer.current = setTimeout(() => setPhase("idle"), 10000);
    } catch (e) {
      setError(errorText(e));
    } finally {
      runningRef.current = false;
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
          await MenuItem.new({
            text: expanded ? t("收起面板") : t("展开面板"),
            enabled: !running,
            action: () => setPhase(expanded ? "idle" : result ? "done" : "plan"),
          }),
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
          await MenuItem.new({ text: t("清空列表"), enabled: !running, action: clear }),
          await MenuItem.new({ text: t("关闭悬浮窗"), action: close }),
        ],
      });
      await menu.popup();
    } catch {
      /* 浏览器预览中没有原生菜单 */
    }
  };

  const drag = (e: MouseEvent) => {
    if ((e.target as Element).closest("button, input, select, a")) return;
    e.stopPropagation();
    if (e.button === 0 && e.detail === 1)
      void getCurrentWindow()
        .startDragging()
        .catch(() => {});
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
        <button className="icon-btn" title={t("收起面板")} onClick={() => setPhase("idle")}>
          <Minus size={15} />
        </button>
        <button className="icon-btn" title={t("关闭悬浮窗")} onClick={close}>
          <X size={15} />
        </button>
      </div>

      <div className={`float-drop-hint ${hover ? "active" : ""}`}>
        <Upload size={13} />
        <span>{hover ? t("松开即可添加文件") : t("可继续拖入文件或文件夹")}</span>
        <button className="icon-btn" title={t("添加文件")} disabled={running} onClick={() => void pickAndHandle()}>
          <Plus size={15} />
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
          plan?.warnings.map((warning, index) => (
            <div key={index} className="banner warn">
              <TriangleAlert size={14} />
              {warning}
            </div>
          ))}
        {phase === "plan" && (items.length > 0 || skipped.length > 0) && (
          <FileList
            items={items}
            skipped={skipped}
            excluded={unchecked}
            view={view}
            onView={changeView}
            onToggle={toggle}
            onRemove={remove}
            onMenu={itemMenu}
            onNewRule={(path) => void newRule(path)}
          />
        )}
        {phase === "plan" && !items.length && !skipped.length && (
          <div className="float-center float-empty">
            <Upload size={28} />
            <span>{t("拖入文件或文件夹")}</span>
            <button className="btn sm" onClick={() => void pickAndHandle()}>
              <Plus size={14} />
              {t("添加文件")}
            </button>
          </div>
        )}
        {phase === "done" && result && (
          <div className="float-center">
            <CircleCheck size={30} color="var(--ok)" />
            <div className="muted" style={{ fontSize: 12.5 }}>
              {undone
                ? t("文件已放回原处")
                : [
                    result.skipped.length && t("跳过 {0} 项", [result.skipped.length]),
                    result.failed.length && t("失败 {0} 项", [result.failed.length]),
                  ]
                    .filter(Boolean)
                    .join(t("，")) || t("全部完成")}
            </div>
          </div>
        )}
      </div>

      <div className="float-foot">
        {(phase === "plan" || phase === "busy") && (
          <button
            className="btn primary"
            style={{ flex: 1 }}
            disabled={chosen.length === 0 || running || phase === "busy"}
            onClick={execute}
          >
            {running ? <Spinner size={14} /> : null}
            {chosen.length ? t("整理 {0} 个文件", [chosen.length]) : items.length ? t("没有勾选要整理的文件") : t("没有可整理的文件")}
          </button>
        )}
        {phase === "done" && !undone && result && result.done > 0 && (
          <button className="btn" style={{ flex: 1 }} onClick={undo}>
            <RotateCcw size={14} /> {t("撤销")}
          </button>
        )}
        <button className="icon-btn float-clear" title={t("清空列表")} disabled={running} onClick={clear}>
          <ListX size={18} />
        </button>
      </div>
    </div>
  );
}
