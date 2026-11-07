import { Code, FolderOpen, Monitor, Moon, Sun, TriangleAlert } from "lucide-react";
import { useEffect, useState, type KeyboardEvent } from "react";
import { api, errorText } from "../api";
import { Segmented } from "../components/ui";
import { useStore } from "../store";
import type { ThemePref } from "../theme";

export default function SettingsPage({
  theme,
  onTheme,
  showAdvanced,
  onShowAdvanced,
}: {
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
