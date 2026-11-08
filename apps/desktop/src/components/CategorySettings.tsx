import { useEffect, useRef, useState } from "react";
import { CircleCheck, Plus, RotateCcw, X } from "lucide-react";
import { api, errorText } from "../api";
import { useStore } from "../store";
import { FileIcon, Spinner } from "./ui";
import { t } from "../i18n";
import { CATEGORY_DROP, type CategoryDrop } from "../categoryDrop";
import { useUnsaved } from "../saveStatus";

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
  const [dropTarget, setDropTarget] = useState<string | null>(null);
  const [pendingDrops, setPendingDrops] = useState(0);
  const [dropNote, setDropNote] = useState<string | null>(null);
  const current = Object.fromEntries(
    categories.map((c) => [c.id, (config.settings?.category_extensions?.[c.id] ?? c.extensions).join(", ")]),
  );
  const values = draft ?? current;
  const valuesRef = useRef(values);
  valuesRef.current = values;
  const dirty = !!draft || Object.values(inputs).some((v) => v.trim());
  useUnsaved("categories", dirty);
  const change = (id: string, value: string) => {
    valuesRef.current = { ...valuesRef.current, [id]: value };
    setDraft(valuesRef.current);
    setSaved(false);
    setError(null);
  };
  const add = (id: string, text = inputs[id] ?? "", fromDrop = false) => {
    if (!text.trim()) return;
    const extensions = parse(text);
    for (const ext of extensions) {
      if (!ext || /[.*/\\:?"<>|]/.test(ext)) {
        setError(t("无效扩展名：{0}", [ext || t("空后缀")]));
        return;
      }
      const owner = categories.find((c) => c.id !== id && parse(valuesRef.current[c.id]).includes(ext));
      if (owner) {
        setError(t("扩展名 .{0} 已属于“{1}”，请先从该类型中移除", [ext, owner.name]));
        return;
      }
    }
    change(id, [...new Set([...parse(valuesRef.current[id]), ...extensions])].join(", "));
    if (!fromDrop) setInputs((prev) => ({ ...prev, [id]: "" }));
    return true;
  };
  const addRef = useRef(add);
  addRef.current = add;
  const busyRef = useRef(busy);
  busyRef.current = busy;
  useEffect(() => {
    let active = true;
    const receive = (event: Event) => {
      const drop = (event as CustomEvent<CategoryDrop>).detail;
      setDropTarget(drop.type === "enter" || drop.type === "over" ? drop.category : null);
      if (drop.type !== "drop" || !drop.category || busyRef.current) return;
      if (!drop.paths.length) {
        setError(t("未能读取拖入的文件，请重新拖入或点击添加文件"));
        return;
      }
      const id = drop.category;
      setPendingDrops((n) => n + 1);
      setError(null);
      setDropNote(null);
      void api
        .droppedExtensions(drop.paths)
        .then((result) => {
          if (!active) return;
          if (!result.extensions.length) {
            setError(t("未找到可添加的扩展名，请拖入带后缀的文件；文件夹不会添加。"));
          } else if (addRef.current(id, result.extensions.join(", "), true)) {
            setDropNote(t("已识别 {0} 种扩展名，跳过 {1} 个项目；点击保存更改后生效。", [result.extensions.length, result.ignored]));
          }
        })
        .catch((e) => {
          if (active) setError(errorText(e));
        })
        .finally(() => {
          if (active) setPendingDrops((n) => n - 1);
        });
    };
    window.addEventListener(CATEGORY_DROP, receive);
    return () => {
      active = false;
      window.removeEventListener(CATEGORY_DROP, receive);
    };
  }, []);
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
      setDropNote(null);
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
        <br />
        {t("也可把文件拖到对应分类，自动提取扩展名；不会移动文件。")}
      </p>
      {categories.map((c) => {
        const extensions = parse(values[c.id]);
        const custom = !same(extensions, c.extensions);
        return (
          <section
            className={`category-setting-row${dropTarget === c.id ? " drop-target" : ""}`}
            data-category-drop={c.id}
            key={c.id}
            aria-labelledby={`category-${c.id}`}
          >
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
              {dropTarget === c.id && <div className="category-drop-hint">{t("松开以添加扩展名")}</div>}
            </div>
          </section>
        );
      })}
      <div className="category-settings-footer">
        {error && (
          <div className="banner error category-feedback" role="alert">
            {error}
          </div>
        )}
        {dropNote && !error && (
          <div className="muted category-feedback" role="status">
            {dropNote}
          </div>
        )}
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
            disabled={busy || pendingDrops > 0 || !dirty}
            onClick={() => {
              setDraft(null);
              setInputs({});
              setError(null);
              setSaved(false);
              setDropNote(null);
            }}
          >
            {t("取消修改")}
          </button>
          <button className="btn sm primary" disabled={busy || pendingDrops > 0 || !dirty || !categories.length} onClick={submit}>
            {(busy || pendingDrops > 0) && <Spinner size={14} />}
            {t("保存更改")}
          </button>
        </div>
      </div>
    </div>
  );
}
