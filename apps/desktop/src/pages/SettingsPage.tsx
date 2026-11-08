import { LANGS, loadLangPref, setLangPref, systemLang, type LangPref } from "../i18n";
import {
  CircleCheck,
  Languages,
  Code,
  Download,
  FolderOpen,
  Monitor,
  Moon,
  RefreshCw,
  Sun,
  TriangleAlert,
  SlidersHorizontal,
  FolderTree,
  Tags,
} from "lucide-react";
import { listen } from "@tauri-apps/api/event";
import { version } from "../../package.json";
import { useEffect, useState, type KeyboardEvent } from "react";
import { api, errorText, type UpdateInfo } from "../api";
import TidyOptions from "../components/TidyOptions";
import CategorySettings from "../components/CategorySettings";
import { Segmented, Spinner, Switch } from "../components/ui";
import { useStore } from "../store";
import type { ThemePref } from "../theme";
import { t } from "../i18n";

export default function SettingsPage({
  update,
  onUpdate,
  theme,
  onTheme,
  showAdvanced,
  onShowAdvanced,
  advancedRequest = 0,
}: {
  update: UpdateInfo | null;
  onUpdate: (u: UpdateInfo) => void;
  theme: ThemePref;
  onTheme: (t: ThemePref) => void;
  showAdvanced: boolean;
  onShowAdvanced: (v: boolean) => void;
  advancedRequest?: number;
}) {
  const { view } = useStore();
  const [section, setSection] = useState(showAdvanced ? "advanced" : "general");
  useEffect(() => {
    if (showAdvanced) setSection("advanced");
  }, [showAdvanced, advancedRequest]);
  const sections = [
    { id: "general", label: t("通用"), Icon: SlidersHorizontal },
    { id: "tidy", label: t("整理"), Icon: FolderTree },
    { id: "categories", label: t("文件分类"), Icon: Tags },
    { id: "updates", label: t("更新"), Icon: Download },
    { id: "advanced", label: t("高级"), Icon: Code },
  ];
  const tabKey = (event: KeyboardEvent<HTMLButtonElement>, index: number) => {
    const next =
      event.key === "ArrowRight"
        ? (index + 1) % sections.length
        : event.key === "ArrowLeft"
          ? (index + sections.length - 1) % sections.length
          : event.key === "Home"
            ? 0
            : event.key === "End"
              ? sections.length - 1
              : -1;
    if (next < 0) return;
    event.preventDefault();
    setSection(sections[next].id);
    document.getElementById(`settings-tab-${sections[next].id}`)?.focus();
  };

  return (
    <div className="page settings-page">
      <div className="page-head">
        <div>
          <h1>{t("设置")}</h1>
        </div>
      </div>

      <div className="settings-tabs" role="tablist" aria-label={t("设置分类")}>
        {sections.map(({ id, label, Icon }, index) => (
          <button
            key={id}
            id={`settings-tab-${id}`}
            role="tab"
            aria-selected={section === id}
            aria-controls={`settings-panel-${id}`}
            tabIndex={section === id ? 0 : -1}
            onKeyDown={(event) => tabKey(event, index)}
            onClick={() => setSection(id)}
          >
            <Icon size={16} />
            <span>{label}</span>
            {id === "updates" && update?.version && <span className="settings-update-dot" />}
          </button>
        ))}
      </div>
      <section
        className="settings-panel"
        id="settings-panel-general"
        role="tabpanel"
        aria-labelledby="settings-tab-general"
        hidden={section !== "general"}
      >
        <div className="card">
          <div className="card-head">
            <h2>{t("通用")}</h2>
          </div>
          <div className="card-body row">
            <span className="text-2" style={{ flex: 1 }}>
              {t("主题")}
            </span>
            <Segmented
              value={theme}
              onChange={onTheme}
              options={[
                {
                  value: "system",
                  label: (
                    <>
                      <Monitor size={14} /> {t("跟随系统")}
                    </>
                  ),
                },
                {
                  value: "light",
                  label: (
                    <>
                      <Sun size={14} /> {t("浅色")}
                    </>
                  ),
                },
                {
                  value: "dark",
                  label: (
                    <>
                      <Moon size={14} /> {t("深色")}
                    </>
                  ),
                },
              ]}
            />
          </div>
          <div className="card-body row" style={{ borderTop: "1px solid var(--border)" }}>
            <span className="text-2" style={{ flex: 1 }}>
              <Languages size={14} style={{ verticalAlign: -2, marginRight: 6 }} />
              {t("语言")}
            </span>
            <select
              className="input"
              style={{ width: "auto" }}
              value={loadLangPref()}
              onChange={(e) => setLangPref(e.target.value as LangPref)}
            >
              <option value="system">
                {t("跟随系统")}（{LANGS.find((l) => l.value === systemLang())?.label}）
              </option>
              {LANGS.map((l) => (
                <option key={l.value} value={l.value}>
                  {l.label}
                </option>
              ))}
            </select>
          </div>
          <AutostartRow />
        </div>
      </section>
      <section
        className="settings-panel"
        id="settings-panel-tidy"
        role="tabpanel"
        aria-labelledby="settings-tab-tidy"
        hidden={section !== "tidy"}
      >
        <TidyOptions />
      </section>
      <section
        className="settings-panel"
        id="settings-panel-categories"
        role="tabpanel"
        aria-labelledby="settings-tab-categories"
        hidden={section !== "categories"}
      >
        <CategorySettings />
      </section>
      <section
        className="settings-panel"
        id="settings-panel-updates"
        role="tabpanel"
        aria-labelledby="settings-tab-updates"
        hidden={section !== "updates"}
      >
        <UpdateCard update={update} onUpdate={onUpdate} />
      </section>
      <section
        className="settings-panel"
        id="settings-panel-advanced"
        role="tabpanel"
        aria-labelledby="settings-tab-advanced"
        hidden={section !== "advanced"}
      >
        <div className="card">
          <div className="card-head">
            <h2>{t("配置文件")}</h2>
            <span className="muted">{t("规则、整理文件夹和固定保留都保存在这里")}</span>
          </div>
          <div className="card-body row">
            <span className="mono ellipsis selectable" style={{ flex: 1 }}>
              {view?.display_path}
            </span>
            {view?.exists && (
              <button className="btn sm" onClick={() => api.reveal(view.path)}>
                <FolderOpen size={14} /> {t("显示")}
              </button>
            )}
            <button className="btn sm" onClick={() => onShowAdvanced(!showAdvanced)}>
              <Code size={14} /> {showAdvanced ? t("收起源码") : t("编辑源码（高级）")}
            </button>
          </div>
          {showAdvanced && <YamlEditor />}
        </div>
      </section>
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
        <h2>{t("版本与更新")}</h2>
        <span className="muted">
          {t("当前版本")} {update?.current ?? version}
        </span>
      </div>
      <div className="card-body" style={{ display: "grid", gap: 10 }}>
        {update?.version ? (
          <>
            <div className="row">
              <Download size={16} color="var(--accent)" />
              <span style={{ flex: 1 }}>
                {t("新版本")} <b>{update.version}</b>
                {update.date && <span className="muted"> · {update.date}</span>}
              </span>
              <button
                className="btn sm primary"
                disabled={!!busy}
                onClick={update.can_install ? install : () => api.openReleases().catch((e) => setError(errorText(e)))}
              >
                {busy === "install" ? <Spinner size={14} /> : <Download size={14} />}
                {busy === "install"
                  ? progress !== null && progress < 100
                    ? t("下载中 {0}%", [progress])
                    : t("正在安装…")
                  : update.can_install
                    ? t("下载并重启")
                    : t("下载安装包")}
              </button>
            </div>
            {update.notes && <div className="update-notes selectable">{update.notes}</div>}
          </>
        ) : (
          <div className="row">
            {checked && !error && <CircleCheck size={16} color="var(--ok)" />}
            <span className="text-2" style={{ flex: 1 }}>
              {checked && !error ? t("已经是最新版本") : t("从 GitHub 检查正式版，验证更新签名后安装，规则和历史都会保留")}
            </span>
            <button className="btn sm" disabled={!!busy} onClick={check}>
              {busy === "check" ? <Spinner size={14} /> : <RefreshCw size={14} />} {t("检查更新")}
            </button>
          </div>
        )}
        {error && <div className="banner error selectable">{error}</div>}
        <div className="row">
          <span className="muted" style={{ flex: 1, fontSize: 12.5 }}>
            {update?.can_install === false
              ? t("当前安装方式需手动安装新版；Linux AppImage 支持应用内更新。")
              : t("只检查正式发布版本，不自动下载或安装。")}
          </span>
          {update?.version && (
            <button className="btn sm" disabled={!!busy} onClick={check}>
              <RefreshCw size={14} />
              {t("检查更新")}
            </button>
          )}
          <button className="btn ghost sm" disabled={!!busy} onClick={() => api.openReleases().catch((e) => setError(errorText(e)))}>
            GitHub Releases
          </button>
        </div>
      </div>
    </div>
  );
}

function AutostartRow() {
  const [on, setOn] = useState<boolean | null>(null);
  const [error, setError] = useState<string | null>(null);
  useEffect(() => {
    api
      .autostartEnabled()
      .then(setOn)
      .catch(() => setOn(null));
  }, []);
  const mac = navigator.userAgent.includes("Mac");
  return (
    <div className="card-body row" style={{ borderTop: "1px solid var(--border)" }}>
      <div style={{ flex: 1 }}>
        <div className="text-2">{t("开机时启动")}</div>
        <div className="muted" style={{ fontSize: 12.5 }}>
          {error ??
            t("只在{0}显示；关掉窗口后 Foldo 也会留在{1}里继续自动整理", [mac ? t("菜单栏") : t("托盘"), mac ? t("菜单栏") : t("托盘")])}
        </div>
      </div>
      <Switch
        label={t("开机时启动")}
        on={!!on}
        disabled={on === null}
        onChange={(v) =>
          api
            .setAutostart(v)
            .then(setOn)
            .catch((e) => setError(errorText(e)))
        }
      />
    </div>
  );
}

function YamlEditor() {
  const { setView } = useStore();
  const [text, setText] = useState("");
  const [saved, setSaved] = useState("");
  const [msg, setMsg] = useState<{ ok: boolean; text: string } | null>(null);

  useEffect(() => {
    api.getYaml().then((yaml) => {
      setText(yaml);
      setSaved(yaml);
    });
  }, []);

  const save = async () => {
    setMsg(null);
    try {
      setView(await api.saveYaml(text));
      setSaved(text);
      setMsg({ ok: true, text: t("已保存") });
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
        {t("直接编辑 YAML 适合熟悉格式的用户。只有通过校验才会保存；在界面里再次修改规则时，手写的注释会被整理掉。")}
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
          {t("放弃修改")}
        </button>
        <button className="btn sm primary" disabled={text === saved} onClick={save}>
          {t("保存（⌘S / Ctrl+S）")}
        </button>
      </div>
    </div>
  );
}
