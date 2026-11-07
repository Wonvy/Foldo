import { useEffect, useState, type KeyboardEvent } from "react";
import { api, errorText, type RulesFile, type RulesSummary } from "../api";
import { Icon } from "../components";

type Validation = { ok: true; summary: RulesSummary } | { ok: false; error: string } | null;

export default function RulesPage({
  file,
  text,
  dirty,
  onChange,
  onSaved,
  onCreated,
  onRevert,
}: {
  file: RulesFile;
  text: string;
  dirty: boolean;
  onChange: (t: string) => void;
  onSaved: (t: string) => void;
  onCreated: (f: RulesFile) => void;
  onRevert: () => void;
}) {
  const [validation, setValidation] = useState<Validation>(null);
  const [saving, setSaving] = useState(false);
  const [message, setMessage] = useState<{ kind: "ok" | "error"; text: string } | null>(null);

  // 输入停顿 300ms 后自动校验
  useEffect(() => {
    if (!file.exists) return;
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .validateRules(text)
        .then((summary) => !cancelled && setValidation({ ok: true, summary }))
        .catch((e) => !cancelled && setValidation({ ok: false, error: errorText(e) }));
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
  }, [text, file.exists]);

  const save = async () => {
    if (!dirty || saving) return;
    setSaving(true);
    setMessage(null);
    try {
      await api.saveRules(text);
      onSaved(text);
      setMessage({ kind: "ok", text: "已保存" });
    } catch (e) {
      setMessage({ kind: "error", text: errorText(e) });
    } finally {
      setSaving(false);
    }
  };

  const onKeyDown = (e: KeyboardEvent<HTMLTextAreaElement>) => {
    if ((e.metaKey || e.ctrlKey) && e.key.toLowerCase() === "s") {
      e.preventDefault();
      void save();
    } else if (e.key === "Tab") {
      // YAML 用空格缩进：Tab 插入两个空格
      e.preventDefault();
      const el = e.currentTarget;
      const { selectionStart: s, selectionEnd: end } = el;
      onChange(text.slice(0, s) + "  " + text.slice(end));
      requestAnimationFrame(() => el.setSelectionRange(s + 2, s + 2));
    }
  };

  if (!file.exists) {
    return (
      <div className="empty">
        <h2>还没有规则文件</h2>
        <p className="muted">规则文件会保存在 {file.display_path}</p>
        <button className="btn primary" onClick={() => api.createExampleRules().then(onCreated)}>
          生成示例规则
        </button>
      </div>
    );
  }

  return (
    <div className="rules-page">
      <header className="page-header">
        <div>
          <h1>规则</h1>
          <button className="path" onClick={() => api.reveal(file.path)} title="在文件管理器中显示">
            {file.display_path} <Icon name="reveal" />
          </button>
        </div>
        <div className="actions">
          {message && <span className={`flash ${message.kind}`}>{message.text}</span>}
          <button className="btn" disabled={!dirty} onClick={onRevert}>
            放弃修改
          </button>
          <button className="btn primary" disabled={!dirty || saving} onClick={save}>
            {saving ? "保存中…" : "保存"}
          </button>
        </div>
      </header>

      <div className="rules-body">
        <aside className="rule-list">
          {validation?.ok === true &&
            (validation.summary.rules.length === 0 ? (
              <p className="muted">还没有规则</p>
            ) : (
              validation.summary.rules.map((r, i) => (
                <div key={r.name} className={`rule-card ${r.enabled ? "" : "disabled"}`}>
                  <div className="rule-name">
                    <span className="rule-index">{i + 1}</span>
                    {r.name}
                    {!r.enabled && <span className="badge">已停用</span>}
                  </div>
                  <div className="rule-meta">
                    {r.locations.join("、")}
                    {r.recursive && "（含子文件夹）"}
                  </div>
                  <div className="rule-meta">{r.actions} 个动作</div>
                </div>
              ))
            ))}
          {validation?.ok === false && <p className="muted">修正错误后显示规则列表</p>}
        </aside>

        <section className="editor-pane">
          <textarea
            className="editor"
            value={text}
            spellCheck={false}
            autoCapitalize="off"
            autoCorrect="off"
            onChange={(e) => onChange(e.target.value)}
            onKeyDown={onKeyDown}
          />
          <div className={`status-bar ${validation?.ok === false ? "error" : ""}`}>
            {validation === null && "校验中…"}
            {validation?.ok === true &&
              `✓ 规则有效：${validation.summary.rules.length} 条规则，${
                validation.summary.rules.filter((r) => r.enabled).length
              } 条启用`}
            {validation?.ok === false && `✗ ${validation.error}`}
            <span className="hint">规则从上到下生效，每个文件只由第一条命中的规则处理 · ⌘S / Ctrl+S 保存</span>
          </div>
        </section>
      </div>
    </div>
  );
}
