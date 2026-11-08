import { useState } from "react";
import { CircleCheck, Plus, RotateCcw, X } from "lucide-react";
import { errorText } from "../api";
import { useStore } from "../store";
import { FileIcon, Spinner } from "./ui";

const parse = (text: string) => [...new Set(text.split(/[\s,，;；]+/).filter(Boolean).map((e) => e.replace(/^\.+/, "").toLowerCase()))];
const same = (a: string[], b: string[]) => a.length === b.length && a.every((e) => b.includes(e));

export default function CategorySettings() {
  const { config, categories, save } = useStore();
  const [draft, setDraft] = useState<Record<string, string> | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const current = Object.fromEntries(categories.map((c) => [c.id,
    (config.settings?.category_extensions?.[c.id] ?? c.extensions).join(", "),
  ]));
  const values = draft ?? current;
  const dirty = !!draft || Object.values(inputs).some((v) => v.trim());
  const change = (id: string, value: string) => {
    setDraft({ ...values, [id]: value });
    setSaved(false);
    setError(null);
  };
  const add = (id: string, text = inputs[id] ?? "") => {
    if (!text.trim()) return;
    const extensions = parse(text);
    for (const ext of extensions) {
      if (!ext || /[.*/\\:?"<>|]/.test(ext)) { setError(`无效扩展名：${ext || "空后缀"}`); return; }
      const owner = categories.find((c) => c.id !== id && parse(values[c.id]).includes(ext));
      if (owner) { setError(`扩展名 .${ext} 已属于“${owner.name}”，请先从该类型中移除`); return; }
    }
    change(id, [...new Set([...parse(values[id]), ...extensions])].join(", "));
    setInputs((prev) => ({ ...prev, [id]: "" }));
  };
  const submit = async () => {
    setBusy(true);
    setError(null);
    try {
      const overrides: Record<string, string[]> = {};
      const owners = new Map<string, string>();
      for (const c of categories) {
        const extensions = parse(`${values[c.id]}, ${inputs[c.id] ?? ""}`);
        for (const ext of extensions) {
          if (!ext || /[.*/\\:?"<>|]/.test(ext)) throw new Error(`无效扩展名：${ext}，请填写最后一个点后的后缀`);
          const owner = owners.get(ext);
          if (owner) throw new Error(`扩展名 ${ext} 同时出现在“${owner}”和“${c.name}”，请只保留一处`);
          owners.set(ext, c.name);
        }
        if (extensions.length !== c.extensions.length || extensions.some((e) => !c.extensions.includes(e))) overrides[c.id] = extensions;
      }
      await save({ ...config, settings: { ...config.settings, category_extensions: overrides } });
      setDraft(null);
      setInputs({});
      setSaved(true);
    } catch (e) {
      setError(errorText(e));
    } finally { setBusy(false); }
  };
  return (
    <div className="card category-settings">
      <div className="card-head"><h2>文件类型与扩展名</h2><span className="spacer" /><span className="muted">{categories.length} 种类型</span></div>
      <p className="category-help muted" id="extension-help">输入后按回车添加，支持粘贴多个扩展名。点击 × 移除，保存后重新扫描生效。</p>
      {categories.map((c) => {
        const extensions = parse(values[c.id]);
        const custom = !same(extensions, c.extensions);
        return (
          <section className="category-setting-row" key={c.id} aria-labelledby={`category-${c.id}`}>
            <div className="category-setting-heading">
              <FileIcon category={c.name} size={32} />
              <div><div className="row-title" id={`category-${c.id}`}>{c.name}</div>
                <div className="category-setting-meta">{extensions.length} 个扩展名{custom && <span> · 已自定义</span>}</div>
              </div>
            </div>
            <div className="category-setting-content">
              <div className="extension-tags">
                {extensions.map((ext) => (
                  <span className={`extension-tag${!c.extensions.includes(ext) ? " custom" : ""}`} key={ext}>
                    <span title={`.${ext}`}>.{ext}</span>
                    <button type="button" disabled={busy} aria-label={`从${c.name}移除 .${ext}`} title={`移除 .${ext}`}
                      onClick={() => change(c.id, extensions.filter((e) => e !== ext).join(", "))}><X size={12} /></button>
                  </span>
                ))}
                <div className="extension-add">
                  <input value={inputs[c.id] ?? ""} disabled={busy} placeholder="添加扩展名" aria-label={`为${c.name}添加扩展名`} aria-describedby="extension-help"
                    onChange={(e) => { setInputs((prev) => ({ ...prev, [c.id]: e.target.value })); setSaved(false); setError(null); }}
                    onKeyDown={(e) => {
                      if (e.nativeEvent.isComposing) return;
                      if (["Enter", ",", "，", ";", "；", " "].includes(e.key)) { e.preventDefault(); add(c.id); }
                    }}
                    onPaste={(e) => {
                      const text = e.clipboardData.getData("text");
                      if (/[\s,，;；]/.test(text.trim())) {
                        e.preventDefault();
                        const input = e.currentTarget;
                        add(c.id, input.value.slice(0, input.selectionStart ?? 0) + text + input.value.slice(input.selectionEnd ?? input.value.length));
                      }
                    }} />
                  <button type="button" disabled={busy || !inputs[c.id]?.trim()} aria-label={`添加到${c.name}`} title="添加扩展名" onClick={() => add(c.id)}><Plus size={14} /></button>
                </div>
              </div>
              {custom && <button type="button" className="category-reset" disabled={busy} onClick={() => {
                change(c.id, c.extensions.join(", "));
                setInputs((prev) => ({ ...prev, [c.id]: "" }));
              }}><RotateCcw size={12} />恢复默认</button>}
            </div>
          </section>
        );
      })}
      {error && <div className="banner error category-error" role="alert">{error}</div>}
      <div className="category-settings-footer">
        <span className={`category-save-status${saved ? " saved" : ""}`} role="status">
          {saved ? <><CircleCheck size={14} />已保存，重新扫描即可生效</> : dirty ? "有未保存的修改" : "扩展名不区分大小写"}
        </span>
        <div className="row">
          <button className="btn sm" disabled={busy || !dirty} onClick={() => { setDraft(null); setInputs({}); setError(null); setSaved(false); }}>取消修改</button>
          <button className="btn sm primary" disabled={busy || !dirty || !categories.length} onClick={submit}>{busy && <Spinner size={14} />}保存更改</button>
        </div>
      </div>
    </div>
  );
}
