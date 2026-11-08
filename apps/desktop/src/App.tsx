import { Clock, FolderTree, PictureInPicture2, Settings, Sparkles, Upload, WandSparkles } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useRef, useState, type ReactNode } from "react";
import { api, type UpdateInfo, type WatchStatus } from "./api";
import HistoryPage from "./pages/HistoryPage";
import RulesPage, { type RuleRequest } from "./pages/RulesPage";
import SettingsPage from "./pages/SettingsPage";
import TidyPage from "./pages/TidyPage";
import { StoreProvider, useStore } from "./store";
import { applyTheme, loadTheme, type ThemePref } from "./theme";

type Page = "tidy" | "rules" | "history" | "settings";

export default function App() {
  return (
    <StoreProvider>
      <Shell />
    </StoreProvider>
  );
}

// 侧栏宽度：可拖动分隔条调整；拖到很窄时收成只有图标
const SIDE_MIN = 160;
const SIDE_MAX = 320;
const SIDE_DEFAULT = 212;
/** 拖到这个宽度以下就收起为图标。 */
const SIDE_COLLAPSE_AT = 120;
const SIDE_KEY = "foldo.sidebar";
type Side = { width: number; collapsed: boolean };

function loadSide(): Side {
  try {
    const v = JSON.parse(localStorage.getItem(SIDE_KEY) ?? "");
    if (typeof v?.width === "number") return { width: Math.min(SIDE_MAX, Math.max(SIDE_MIN, v.width)), collapsed: !!v.collapsed };
  } catch {
    /* 没有保存过或存储不可用 */
  }
  return { width: SIDE_DEFAULT, collapsed: false };
}

function saveSide(s: Side) {
  try {
    localStorage.setItem(SIDE_KEY, JSON.stringify(s));
  } catch {
    /* 存储不可用 */
  }
}

function Shell() {
  const { config, loadError, reload } = useStore();
  const [page, setPage] = useState<Page>("tidy");
  const [theme, setTheme] = useState<ThemePref>(loadTheme);
  const [dragging, setDragging] = useState(false);
  const [dropped, setDropped] = useState<{ paths: string[]; n: number } | null>(null);
  const [rulesDrop, setRulesDrop] = useState<{ paths: string[]; n: number } | null>(null);
  const [floatOn, setFloatOn] = useState(false);
  const [side, setSide] = useState<Side>(loadSide);
  const [resizing, setResizing] = useState(false);
  const pageRef = useRef<Page>("tidy");
  pageRef.current = page;
  const [showAdvanced, setShowAdvanced] = useState(false);
  const [ruleRequest, setRuleRequest] = useState<RuleRequest | null>(null);
  const [scanRequest, setScanRequest] = useState(0);
  const [watch, setWatch] = useState<WatchStatus | null>(null);
  const [update, setUpdate] = useState<UpdateInfo | null>(null);
  const watchRef = useRef<WatchStatus | null>(null);
  watchRef.current = watch;

  useEffect(() => applyTheme(theme), [theme]);

  // 悬浮窗是否开着（关闭悬浮窗时也会同步，见下方的轮询）
  useEffect(() => {
    const check = () => api.floatVisible().then(setFloatOn).catch(() => {});
    check();
    const t = setInterval(check, 2000);
    return () => clearInterval(t);
  }, []);

  // 启动时悄悄检查一次更新；没联网或还没有发布版本时不提示
  useEffect(() => {
    api.checkUpdate().then(setUpdate).catch(() => {});
  }, []);

  // 后台自动整理的状态，以及托盘发来的事件
  useEffect(() => {
    api.watchStatus().then(setWatch).catch(() => {});
    const uns: (() => void)[] = [];
    let disposed = false;
    (async () => {
      try {
        const add = (u: () => void) => (disposed ? u() : uns.push(u));
        add(await listen<WatchStatus>("watch-status", (e) => setWatch(e.payload)));
        add(await listen("config-changed", () => void reload()));
        const scanNow = () => {
          setPage("tidy");
          setScanRequest((n) => n + 1);
        };
        add(await listen("tidy-scan", scanNow));
        // 托盘里选了“自动整理”：到整理页确认（确认框由整理页顶部的模式切换显示）
        add(await listen("confirm-auto", () => setPage("tidy")));
        // 点了托盘状态或系统通知：有待确认的文件就去整理页，刚自动整理过就看历史
        add(
          await listen("open-pending", () => {
            const w = watchRef.current;
            if (!w?.pending && w?.last_batch) setPage("history");
            else scanNow();
          }),
        );
      } catch {
        /* 浏览器预览中没有事件 */
      }
    })();
    return () => {
      disposed = true;
      uns.forEach((u) => u());
    };
  }, [reload]);

  // 把文件拖进窗口：像 DropIt 一样，按规则处理这些文件
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    (async () => {
      try {
        const { getCurrentWebview } = await import("@tauri-apps/api/webview");
        const un = await getCurrentWebview().onDragDropEvent((e) => {
          const p = e.payload;
          if (p.type === "enter" || p.type === "over") setDragging(true);
          else if (p.type === "leave") setDragging(false);
          else if (p.type === "drop") {
            setDragging(false);
            if (p.paths.length) {
              // 在规则页拖入：用来调试规则；其他页面：按规则整理这些文件
              if (pageRef.current === "rules") {
                setRulesDrop({ paths: p.paths, n: Date.now() });
              } else {
                setPage("tidy");
                setDropped({ paths: p.paths, n: Date.now() });
              }
            }
          }
        });
        if (disposed) un();
        else unlisten = un;
      } catch {
        /* 浏览器预览中没有拖放事件 */
      }
    })();
    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);

  // 拖动分隔条
  useEffect(() => {
    if (!resizing) return;
    const move = (e: PointerEvent) => {
      const x = e.clientX;
      setSide((s) => (x < SIDE_COLLAPSE_AT ? { ...s, collapsed: true } : { width: Math.min(SIDE_MAX, Math.max(SIDE_MIN, x)), collapsed: false }));
    };
    const up = () => setResizing(false);
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", up, { once: true });
    return () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", up);
    };
  }, [resizing]);
  useEffect(() => {
    if (!resizing) saveSide(side);
  }, [side, resizing]);

  const ruleCount = config.rules?.length ?? 0;
  const nav: { id: Page; label: string; icon: ReactNode; count?: number | string }[] = [
    { id: "tidy", label: "整理", icon: <Sparkles size={17} />, count: watch?.pending || undefined },
    { id: "rules", label: "规则", icon: <WandSparkles size={17} />, count: ruleCount || undefined },
    { id: "history", label: "历史", icon: <Clock size={17} /> },
    { id: "settings", label: "设置", icon: <Settings size={17} />, count: update?.version ? "新" : undefined },
  ];

  return (
    <div className={`app ${resizing ? "resizing" : ""}`}>
      <nav className={`sidebar ${side.collapsed ? "collapsed" : ""}`} style={side.collapsed ? undefined : { width: side.width }}>
        <div className="brand">
          <span className="brand-mark">
            <FolderTree size={16} />
          </span>
          <span className="label">Foldo</span>
        </div>
        {nav.map((n) => (
          <button
            key={n.id}
            className={`nav-item ${page === n.id ? "active" : ""}`}
            onClick={() => setPage(n.id)}
            title={n.label}
          >
            {n.icon}
            <span className="label">{n.label}</span>
            {n.count !== undefined && <span className="count">{n.count}</span>}
          </button>
        ))}
        <div className="sidebar-foot">
          <button
            className={`icon-btn ${floatOn ? "active" : ""}`}
            title={floatOn ? "关闭悬浮窗" : "打开悬浮窗（把文件拖上去就能整理）"}
            onClick={() => api.toggleFloat().then(setFloatOn).catch(() => {})}
          >
            <PictureInPicture2 size={16} />
          </button>
          <span className="muted label" style={{ fontSize: 12 }}>
            v0.1
          </span>
        </div>
      </nav>
      <div
        className={`splitter ${resizing ? "active" : ""}`}
        title="拖动调整宽度，双击切换图标模式"
        onPointerDown={(e) => {
          if (e.button !== 0) return;
          e.preventDefault();
          setResizing(true);
        }}
        onDoubleClick={() => setSide((s) => ({ ...s, collapsed: !s.collapsed }))}
      />

      <main className="content">
        {loadError && <div className="banner error" style={{ margin: 24 }}>读取配置失败：{loadError}</div>}
        <div hidden={page !== "tidy"}>
          <TidyPage
            dropped={dropped}
            scanRequest={scanRequest}
            watch={watch}
            onNewRule={(folder) => {
              setPage("rules");
              setRuleRequest({ n: Date.now(), assignTo: folder });
            }}
            onEditRule={(name) => {
              setPage("rules");
              setRuleRequest({ n: Date.now(), edit: name });
            }}
          />
        </div>
        <div hidden={page !== "rules"}>
          <RulesPage
            request={ruleRequest}
            dropped={rulesDrop}
            onOpenAdvanced={() => {
              setShowAdvanced(true);
              setPage("settings");
            }}
          />
        </div>
        <div hidden={page !== "history"}>
          <HistoryPage active={page === "history"} />
        </div>
        <div hidden={page !== "settings"}>
          <SettingsPage update={update} onUpdate={setUpdate} theme={theme} onTheme={setTheme} showAdvanced={showAdvanced} onShowAdvanced={setShowAdvanced} />
        </div>
      </main>

      {dragging && (
        <div className="drop-overlay">
          <div className="box">
            <Upload size={28} color="var(--accent)" />
            <h2 style={{ marginTop: 10 }}>{page === "rules" ? "松开以调试规则" : "松开即可整理这些文件"}</h2>
            <p className="muted" style={{ marginTop: 4 }}>
              {page === "rules" ? "看它会被哪条规则处理，不会改动文件" : "按你的规则给出整理建议，确认后再执行"}
            </p>
          </div>
        </div>
      )}
    </div>
  );
}
