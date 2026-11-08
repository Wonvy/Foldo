import { Bell, CircleCheck, Code, Download, FolderOpen, Monitor, Moon, PauseCircle, RefreshCw, Sun, TriangleAlert, Zap } from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { useEffect, useState, type KeyboardEvent } from "react";
import { api, errorText, type AutoMode, type UpdateInfo, type WatchStatus } from "../api";
import { Segmented, Spinner, Switch } from "../components/ui";
import { useStore } from "../store";
import type { ThemePref } from "../theme";

export default function SettingsPage({
  watch,
  update,
  onUpdate,
  theme,
  onTheme,
  showAdvanced,
  onShowAdvanced,
}: {
  watch: WatchStatus | null;
  update: UpdateInfo | null;
  onUpdate: (u: UpdateInfo) => void;
  theme: ThemePref;
  onTheme: (t: ThemePref) => void;
  showAdvanced: boolean;
  onShowAdvanced: (v: boolean) => void;
}) {
  const { view } = useStore();

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>设置</h1>
        </div>
      </div>

      <AutoCard watch={watch} />

      <div className="card">
        <div className="card-head">
          <h2>外观</h2>
        </div>
        <div className="card-body row">
          <span className="text-2" style={{ width: 72 }}>
            主题
          </span>
          <Segmented
            value={theme}
            onChange={onTheme}
            options={[
              { value: "system", label: <><Monitor size={14} /> 跟随系统</> },
              { value: "light", label: <><Sun size={14} /> 浅色</> },
              { value: "dark", label: <><Moon size={14} /> 深色</> },
            ]}
          />
        </div>
      </div>

      <UpdateCard update={update} onUpdate={onUpdate} />

      <div className="card">
        <div className="card-head">
          <h2>配置文件</h2>
          <span className="muted">规则、整理文件夹和固定保留都保存在这里</span>
        </div>
        <div className="card-body row">
          <span className="mono ellipsis selectable" style={{ flex: 1 }}>
            {view?.display_path}
          </span>
          {view?.exists && (
            <button className="btn sm" onClick={() => api.reveal(view.path)}>
              <FolderOpen size={14} /> 显示
            </button>
          )}
          <button className="btn sm" onClick={() => onShowAdvanced(!showAdvanced)}>
            <Code size={14} /> {showAdvanced ? "收起源码" : "编辑源码（高级）"}
          </button>
        </div>
        {showAdvanced && <YamlEditor />}
      </div>
    </div>
  );
}

function UpdateCard({ update, onUpdate }: { update: UpdateInfo | null; onUpdate: (u: UpdateInfo) => void }) {
  const [busy, setBusy] = useState<null | "check" | "install">(null);
  const [checked, setChecked] = useState(false);
  const [progress, setProgress] = useState<number | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let un: (() => void) | undefined;
    let disposed = false;
    listen<{ downloaded: number; total: number | null }>("update-progress", (e) => {
      const { downloaded, total } = e.payload;
      setProgress(total ? Math.min(100, Math.round((downloaded / total) * 100)) : null);
    })
      .then((u) => (disposed ? u() : (un = u)))
      .catch(() => {});
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  const check = async () => {
    setBusy("check");
    setError(null);
    try {
      onUpdate(await api.checkUpdate());
      setChecked(true);
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const install = async () => {
    setBusy("install");
    setError(null);
    setProgress(0);
    try {
      await api.installUpdate(); // 成功后应用会重启
    } catch (e) {
      setError(errorText(e));
      setBusy(null);
      setProgress(null);
    }
  };

  return (
    <div className="card">
      <div className="card-head">
        <h2>版本与更新</h2>
        {update && <span className="muted">当前版本 {update.current}</span>}
      </div>
      <div className="card-body" style={{ display: "grid", gap: 10 }}>
        {update?.version ? (
          <>
            <div className="row">
              <Download size={16} color="var(--accent)" />
              <span style={{ flex: 1 }}>
                新版本 <b>{update.version}</b>
                {update.date && <span className="muted"> · {update.date}</span>}
              </span>
              <button className="btn sm primary" disabled={!!busy} onClick={install}>
                {busy === "install" ? <Spinner size={14} /> : <Download size={14} />}
                {busy === "install" ? (progress !== null ? `下载中 ${progress}%` : "正在安装…") : "下载并重启"}
              </button>
            </div>
            {update.notes && <div className="update-notes selectable">{update.notes}</div>}
          </>
        ) : (
          <div className="row">
            {checked && !error && <CircleCheck size={16} color="var(--ok)" />}
            <span className="text-2" style={{ flex: 1 }}>
              {checked && !error ? "已经是最新版本" : "新版本发布在 GitHub，安装后自动替换旧版本，规则和历史都会保留"}
            </span>
            <button className="btn sm" disabled={!!busy} onClick={check}>
              {busy === "check" ? <Spinner size={14} /> : <RefreshCw size={14} />} 检查更新
            </button>
          </div>
        )}
        {error && <div className="banner error selectable">{error}</div>}
      </div>
    </div>
  );
}

const AUTO_HINT: Record<AutoMode, string> = {
  off: "只在你打开 Foldo 时整理",
  notify: "有可以整理的文件时发通知，由你确认",
  run: "按规则直接整理，可在历史里撤销；移到回收站仍需你确认",
};

function AutoCard({ watch }: { watch: WatchStatus | null }) {
  const { config, save } = useStore();
  const mode: AutoMode = config.settings?.auto ?? "off";
  const [autostart, setAutostart] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    api.autostartEnabled().then(setAutostart).catch(() => setAutostart(null));
  }, []);

  const setMode = async (auto: AutoMode) => {
    setError(null);
    try {
      await save({ ...config, settings: { ...config.settings, auto: auto === "off" ? undefined : auto } });
    } catch (e) {
      setError(String(e));
    }
  };

  const status = (() => {
    if (!watch || mode === "off") return null;
    if (watch.error) return <span style={{ color: "var(--danger)" }}>{watch.error}</span>;
    const parts: string[] = [];
    if (watch.last_check) parts.push(`${watch.last_check} 检查过`);
    if (watch.pending) parts.push(mode === "run" ? `${watch.pending} 个文件等你确认移到回收站` : `${watch.pending} 个文件可以整理`);
    if (watch.last_done) parts.push(`上次自动整理了 ${watch.last_done} 个`);
    return parts.length ? parts.join(" · ") : "正在监听…";
  })();

  return (
    <div className="card">
      <div className="card-head">
        <h2>后台自动整理</h2>
        <span className="muted">关掉窗口后 Foldo 留在{navigator.userAgent.includes("Mac") ? "菜单栏" : "托盘"}里继续工作</span>
      </div>
      <div className="card-body" style={{ display: "grid", gap: 12 }}>
        <div className="row">
          <Segmented
            value={mode}
            onChange={(v) => void setMode(v)}
            options={[
              { value: "off", label: <><PauseCircle size={14} /> 关闭</> },
              { value: "notify", label: <><Bell size={14} /> 先提醒我</> },
              { value: "run", label: <><Zap size={14} /> 直接整理</> },
            ]}
          />
          <span className="muted" style={{ fontSize: 12.5 }}>{AUTO_HINT[mode]}</span>
        </div>
        {status && (
          <div className="row muted" style={{ fontSize: 12.5 }}>
            <span className="dot-live" />
            {status}
          </div>
        )}
        <div className="row">
          <span className="text-2" style={{ flex: 1 }}>开机时启动（只在{navigator.userAgent.includes("Mac") ? "菜单栏" : "托盘"}显示）</span>
          <Switch
            label="开机时启动"
            on={!!autostart}
            disabled={autostart === null}
            onChange={(v) => api.setAutostart(v).then(setAutostart).catch((e) => setError(errorText(e)))}
          />
        </div>
        {error && <div className="banner error selectable">{error}</div>}
      </div>
    </div>
  );
}

function YamlEditor() {
  const { setView } = useStore();
  const [text, setText] = useState("");
  const [saved, setSaved] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    api.getYaml().then((t) => {
      setText(t);
      setSaved(t);
    });
  }, []);

  const save = async () => {
    setMsg(null);
    try {
      setView(await api.saveYaml(text));
      setSaved(text);
      setMsg({ ok: true, text: "已保存" });
    } catch (e) {
      setMsg({ ok: false, text: errorText(e) });
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void save();
    } else if (e.key === "Tab") {
      e.preventDefault();
      const el = e.currentTarget;
      const { selectionStart: s, selectionEnd: end } = el;
      setText(text.slice(0, s) + "  " + text.slice(end));
      requestAnimationFrame(() => el.setSelectionRange(s + 2, s + 2));
    }
  };

  return (
    <div style={{ borderTop: "1px solid var(--border)" }}>
      <div className="banner warn" style={{ margin: 12 }}>
        <TriangleAlert size={16} />
        直接编辑 YAML 适合熟悉格式的用户。只有通过校验才会保存；在界面里再次修改规则时，手写的注释会被整理掉。
      </div>
      <textarea
        className="editor selectable"
        spellCheck={false}
        value={text}
        onChange={(e) => setText(e.target.value)}
        onKeyDown={onKeyDown}
      />
      <div className="row" style={{ padding: "10px 16px", borderTop: "1px solid var(--border)" }}>
        {msg && (
          <span style={{ color: msg.ok ? "var(--ok)" : "var(--danger)", flex: 1 }} className="selectable">
            {msg.text}
          </span>
        )}
        <span className="spacer" />
        <button className="btn sm" disabled={text === saved} onClick={() => setText(saved)}>
          放弃修改
        </button>
        <button className="btn sm primary" disabled={text === saved} onClick={save}>
          保存（⌘S / Ctrl+S）
        </button>
      </div>
    </div>
  );
}
