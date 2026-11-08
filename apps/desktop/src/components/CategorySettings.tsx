import { useState } from "react";
import { CircleCheck, Plus, RotateCcw, X } from "lucide-react";
import { errorText } from "../api";
import { useStore } from "../store";
import { FileIcon, Spinner } from "./ui";
import { t } from "../i18n";

const parse = (text: string) => [
  ...new Set(
    text
      .split(/[\s,，、;；]+/)
      .filter(Boolean)
      .map((e) => e.replace(/^\.+/, "").toLowerCase()),
  ),
];
const same = (a: string[], b: string[]) => a.length === b.length && a.every((e) => b.includes(e));

export default function CategorySettings() {
  const { config, categories, save } = useStore();
  const [draft, setDraft] = useState<Record<string, string> | null>(null);
  const [busy, setBusy] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saved, setSaved] = useState(false);
  const [inputs, setInputs] = useState<Record<string, string>>({});
  const current = Object.fromEntries(
    categories.map((c) => [c.id, (config.settings?.category_extensions?.[c.id] ?? c.extensions).join(", ")]),
  );
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
      if (!ext || /[.*/\\:?"<>|]/.test(ext)) {
        setError(t("无效扩展名：{0}", [ext || t("空后缀")]));
        return;
      }
      const owner = categories.find((c) => c.id !== id && parse(values[c.id]).includes(ext));
      if (owner) {
        setError(t("扩展名 .{0} 已属于“{1}”，请先从该类型中移除", [ext, owner.name]));
        return;
      }
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
          if (!ext || /[.*/\\:?"<>|]/.test(ext)) throw new Error(t("无效扩展名：{0}，请填写最后一个点后的后缀", [ext]));
          const owner = owners.get(ext);
          if (owner) throw new Error(t("扩展名 {0} 同时出现在“{1}”和“{2}”，请只保留一处", [ext, owner, c.name]));
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
    } finally {
      setBusy(false);
    }
  };
  return (
    <div className="card category-settings">
      <div className="card-head">
        <h2>{t("文件类型与扩展名")}</h2>
        <span className="spacer" />
        <span className="muted">{t("{0} 种类型", [categories.length])}</span>
      </div>
      <p className="category-help muted" id="extension-help">
        {t("输入后按回车添加，支持粘贴多个扩展名。点击 × 移除，保存后重新扫描生效。")}
      </p>
      {categories.map((c) => {
        const extensions = parse(values[c.id]);
        const custom = !same(extensions, c.extensions);
        return (
          <section className="category-setting-row" key={c.id} aria-labelledby={`category-${c.id}`}>
            <div className="category-setting-heading">
              <FileIcon category={c.name} size={32} />
              <div>
                <div className="row-title" id={`category-${c.id}`}>
                  {t(c.name)}
                </div>
                <div className="category-setting-meta">
                  {t("{0} 个扩展名", [extensions.length])}
                  {custom && <span> {t("· 已自定义")}</span>}
                </div>
              </div>
            </div>
            <div className="category-setting-content">
              <div className="extension-tags">
                {extensions.map((ext) => (
                  <span className={`extension-tag${!c.extensions.includes(ext) ? " custom" : ""}`} key={ext}>
                    <span title={`.${ext}`}>.{ext}</span>
                    <button
                      type="button"
                      disabled={busy}
                      aria-label={t("从{0}移除 .{1}", [c.name, ext])}
                      title={t("移除 .{0}", [ext])}
                      onClick={() => change(c.id, extensions.filter((e) => e !== ext).join(", "))}
                    >
                      <X size={12} />
                    </button>
                  </span>
                ))}
                <div className="extension-add">
                  <input
                    value={inputs[c.id] ?? ""}
                    disabled={busy}
                    placeholder={t("添加扩展名")}
                    aria-label={t("为{0}添加扩展名", [c.name])}
                    aria-describedby="extension-help"
                    onChange={(e) => {
                      setInputs((prev) => ({ ...prev, [c.id]: e.target.value }));
                      setSaved(false);
                      setError(null);
                    }}
                    onKeyDown={(e) => {
                      if (e.nativeEvent.isComposing) return;
                      if (["Enter", ",", "，", "、", ";", "；", " "].includes(e.key)) {
                        e.preventDefault();
                        add(c.id);
                      }
                    }}
                    onPaste={(e) => {
                      const text = e.clipboardData.getData("text");
                      if (/[\s,，、;；]/.test(text.trim())) {
                        e.preventDefault();
                        const input = e.currentTarget;
                        add(
                          c.id,
                          input.value.slice(0, input.selectionStart ?? 0) +
                            text +
                            input.value.slice(input.selectionEnd ?? input.value.length),
                        );
                      }
                    }}
                  />
                  <button
                    type="button"
                    disabled={busy || !inputs[c.id]?.trim()}
                    aria-label={t("添加到{0}", [c.name])}
                    title={t("添加扩展名")}
                    onClick={() => add(c.id)}
                  >
                    <Plus size={14} />
                  </button>
                </div>
              </div>
              {custom && (
                <button
                  type="button"
                  className="category-reset"
                  disabled={busy}
                  onClick={() => {
                    change(c.id, c.extensions.join(", "));
                    setInputs((prev) => ({ ...prev, [c.id]: "" }));
                  }}
                >
                  <RotateCcw size={12} />
                  {t("恢复默认")}
                </button>
              )}
            </div>
          </section>
        );
      })}
      {error && (
        <div className="banner error category-error" role="alert">
          {error}
        </div>
      )}
      <div className="category-settings-footer">
        <span className={`category-save-status${saved ? " saved" : ""}`} role="status">
          {saved ? (
            <>
              <CircleCheck size={14} />
              {t("已保存，重新扫描即可生效")}
            </>
          ) : dirty ? (
            t("有未保存的修改")
          ) : (
            t("扩展名不区分大小写")
          )}
        </span>
        <div className="row">
          <button
            className="btn sm"
            disabled={busy || !dirty}
            onClick={() => {
              setDraft(null);
              setInputs({});
              setError(null);
              setSaved(false);
            }}
          >
            {t("取消修改")}
          </button>
          <button className="btn sm primary" disabled={busy || !dirty || !categories.length} onClick={submit}>
            {busy && <Spinner size={14} />}
            {t("保存更改")}
          </button>
        </div>
      </div>
    </div>
  );
}
