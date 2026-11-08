// 新建 / 编辑规则：“如果 … 就 …”。规则完全由用户填写，“用文件测试”只用来检查。
import { ChevronRight, Code, File as FileIcon2, Folder as FolderIcon, Plus, Trash, TriangleAlert } from "lucide-react";
import { useEffect, useMemo, useState } from "react";
import { api, errorText, type RuleDef, type RuleTarget, type TestView } from "../api";
import {
  DEFAULT_FIELD,
  emptyForm,
  fieldFits,
  formToRule,
  group,
  newTest,
  renameInFolders,
  ruleToForm,
  type CondNode,
  type FieldMap,
  type RuleForm,
} from "../rules";
import { pickFile, pickFolder, useStore } from "../store";
import GroupEditor, { mapResults, type ResultMap } from "./ConditionTree";
import InspectDialog from "./InspectDialog";
import { Modal, Segmented, Spinner, Switch } from "./ui";
import ActionList from "./rule-editor/ActionList";
import { MatchChip, TestChip } from "./rule-editor/FooterChips";
import { t } from "../i18n";

/** 切换处理对象：用不了的条件换成新对象的默认条件；文件夹不能“解压”，换成“移动”。 */
function switchTarget(form: RuleForm, target: RuleTarget, fieldMap: FieldMap): RuleForm {
  const fits = (n: CondNode): boolean => (n.kind === "group" ? n.children.every(fits) : fieldFits(fieldMap.get(n.field), target));
  return {
    ...form,
    target,
    root: fits(form.root) ? form.root : group("all", [newTest(fieldMap, DEFAULT_FIELD[target])]),
    exceptions: fits(form.exceptions) ? form.exceptions : group("any"),
    actions: form.actions.map((a) => (target === "folder" && a.kind === "extract" ? { ...a, kind: "move", to: "" } : a)),
  };
}

export default function RuleEditor({
  index,
  testDrop,
  assignTo,
  onClose,
  onOpenAdvanced,
}: {
  /** 新建时：保存后把规则加入这个文件夹。 */
  assignTo?: string | string[];
  /** 编辑第几条规则；null 表示新建。 */
  index: number | null;
  /** 拖进面板的文件：直接作为测试文件。 */
  testDrop?: { path: string; n: number } | null;
  onClose: () => void;
  onOpenAdvanced: () => void;
}) {
  const { config, save, fields, fieldMap } = useStore();
  const rules = config.rules ?? [];
  const original = index === null ? null : rules[index];
  const ready = fields.length > 0;
  const initial = useMemo(
    () => (ready ? (original ? ruleToForm(original, fieldMap) : emptyForm(fieldMap)) : null),
    [original, ready, fieldMap],
  );
  const [form, setForm] = useState<RuleForm | null>(initial);
  useEffect(() => {
    if (!form && initial) setForm(initial);
  }, [initial, form]);

  const [showMore, setShowMore] = useState(false);
  const [showErrors, setShowErrors] = useState(false);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);
  const [preview, setPreview] = useState<{ count: number; names: string[] } | null>(null);
  const [testPath, setTestPath] = useState<string | null>(null);
  const [testView, setTestView] = useState<TestView | null>(null);
  const [previewError, setPreviewError] = useState<string | null>(null);
  const [testError, setTestError] = useState<string | null>(null);
  const [inspecting, setInspecting] = useState(false);

  useEffect(() => {
    if (testDrop) setTestPath(testDrop.path);
  }, [testDrop]);

  const set = (patch: Partial<RuleForm>) => setForm((f) => f && { ...f, ...patch });
  const built = form ? formToRule({ ...form, name: form.name || t("未命名") }, fieldMap) : null;
  const rule = built && "rule" in built ? built.rule : null;
  const key = JSON.stringify(rule);

  // 整理文件夹里有多少文件符合
  useEffect(() => {
    setPreview(null);
    setPreviewError(null);
    if (!rule) return setPreview(null);
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .tryRule({ ...rule, name: "__试运行__", enabled: true }, null)
        .then((p) => {
          if (cancelled) return;
          const names = [
            ...new Set([...p.ops.map((o) => o.origin_name), ...p.skipped.filter((s) => s.rule === "__试运行__").map((s) => s.name)]),
          ];
          setPreview({ count: p.matched, names: names.slice(0, 8) });
        })
        .catch((e) => !cancelled && setPreviewError(errorText(e)));
    }, 450);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key]);

  // 用选定的文件测试
  useEffect(() => {
    setTestView(null);
    setTestError(null);
    if (!rule || !testPath) return setTestView(null);
    let cancelled = false;
    const timer = setTimeout(() => {
      api
        .testRule({ ...rule, enabled: true }, testPath)
        .then((v) => !cancelled && setTestView(v))
        .catch((e) => !cancelled && setTestError(errorText(e)));
    }, 300);
    return () => {
      cancelled = true;
      clearTimeout(timer);
    };
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [key, testPath]);

  const results = useMemo<ResultMap | undefined>(() => {
    if (!testView || !form) return undefined;
    const m: ResultMap = new Map();
    if (testView.when) mapResults(form.root, testView.when, m);
    form.exceptions.children.forEach((c, i) => mapResults(c, testView.exceptions[i], m));
    return m;
  }, [testView, form]);

  const submit = async () => {
    if (!form) return;
    const b = formToRule(form, fieldMap);
    if ("error" in b) {
      setShowErrors(true);
      setError(b.error);
      return;
    }
    setSaving(true);
    setError(null);
    const next: RuleDef[] = index === null ? [...rules, b.rule] : rules.map((r, i) => (i === index ? b.rule : r));
    // 改名时同步文件夹里的引用；新建时加入指定的文件夹
    const base =
      index === null
        ? {
            ...config,
            folders: (config.folders ?? []).map((f) =>
              (Array.isArray(assignTo) ? assignTo.includes(f.path) : f.path === assignTo)
                ? { ...f, rules: [...(f.rules ?? []), b.rule.name] }
                : f,
            ),
          }
        : renameInFolders(config, rules[index].name, b.rule.name);
    try {
      await save({ ...base, rules: next });
      onClose();
    } catch (e) {
      setError(errorText(e));
      setSaving(false);
    }
  };

  const remove = async () => {
    if (index === null) return;
    try {
      await save({ ...config, rules: rules.filter((_, i) => i !== index) });
      onClose();
    } catch (e) {
      setError(errorText(e));
    }
  };

  if (!form) {
    return (
      <Modal title={index === null ? t("新建规则") : t("编辑规则")} onClose={onClose}>
        <div className="row muted" style={{ paddingBottom: 16 }}>
          <Spinner />
        </div>
      </Modal>
    );
  }

  if (form.advanced) {
    return (
      <Modal title={original?.name ?? t("编辑规则")} onClose={onClose}>
        <div className="banner warn">
          <TriangleAlert size={16} />
          {t("这条规则会覆盖同名文件，请在源码里修改。")}
        </div>
        <div className="row" style={{ marginBottom: 16 }}>
          <span className="spacer" />
          <button className="btn primary" onClick={() => (onClose(), onOpenAdvanced())}>
            <Code size={15} /> {t("打开源码")}
          </button>
        </div>
      </Modal>
    );
  }

  // 只列出适用于当前处理对象的属性
  const targetFields = fields.filter((f) => fieldFits(f, form.target));

  const moreSummary = [form.exceptions.children.length > 0 && t("{0} 个例外", [form.exceptions.children.length])]
    .filter(Boolean)
    .join(" · ");

  return (
    <Modal
      title={
        <span className="title-row">
          <input
            className={`title-input ${showErrors && !form.name.trim() ? "invalid" : ""}`}
            value={form.name}
            placeholder={index === null ? t("新规则") : t("规则名称")}
            onChange={(e) => set({ name: e.target.value })}
          />
          <span title={form.enabled ? t("已启用") : t("已停用")}>
            <Switch on={form.enabled} onChange={(enabled) => set({ enabled })} label={t("启用")} />
          </span>
        </span>
      }
      onClose={onClose}
      width={760}
      footer={
        <>
          <MatchChip preview={preview} invalid={!rule} error={previewError} />
          <TestChip
            path={testPath}
            view={testView}
            error={testError ?? (!rule ? t("请先补全规则条件和动作") : null)}
            onPick={async () => {
              const p =
                form.target === "folder" ? await pickFolder(t("选择一个用来测试的文件夹")) : await pickFile(t("选择一个用来测试的文件"));
              if (p) setTestPath(p);
            }}
            onInspect={() => setInspecting(true)}
            onClear={() => setTestPath(null)}
          />
          <span className="spacer" />
          {index !== null && (
            <button
              className="icon-btn"
              title={t("删除规则")}
              aria-label={t("删除规则")}
              onClick={remove}
              style={{ color: "var(--danger)" }}
            >
              <Trash size={16} />
            </button>
          )}
          <button className="btn" onClick={onClose}>
            {t("取消")}
          </button>
          <button data-testid="rule-save" className="btn primary" disabled={saving} onClick={submit}>
            {saving && <Spinner size={14} />}
            {t("保存")}
          </button>
        </>
      }
    >
      <section className="rule-block">
        <span className="block-tag if">{t("如果")}</span>
        <div className="block-body">
          <div className="target-row">
            <Segmented
              value={form.target}
              onChange={(tg) => setForm(switchTarget(form, tg, fieldMap))}
              options={[
                {
                  value: "file",
                  label: (
                    <>
                      <FileIcon2 size={14} /> {t("文件")}
                    </>
                  ),
                },
                {
                  value: "folder",
                  label: (
                    <>
                      <FolderIcon size={14} /> {t("文件夹")}
                    </>
                  ),
                },
              ]}
            />
            <span className="muted">
              {form.target === "folder" ? t("整理文件夹第一层的子文件夹，整个文件夹一起处理") : t("整理文件夹里的文件")}
            </span>
          </div>
          <GroupEditor node={form.root} onChange={(root) => set({ root })} fields={targetFields} fieldMap={fieldMap} results={results} />
        </div>
      </section>

      <section className="rule-block">
        <span className="block-tag then">{t("就")}</span>
        <div className="block-body">
          <ActionList rows={form.actions} folder={form.target === "folder"} onChange={(actions) => set({ actions })} />
        </div>
      </section>

      <section className="more">
        <button className="more-toggle" onClick={() => setShowMore(!showMore)}>
          <ChevronRight size={15} className={showMore ? "rot" : ""} />
          {t("更多选项")}
          {!showMore && <span className="muted">{moreSummary}</span>}
        </button>
        {showMore && (
          <div className="more-body">
            <div className="opt-row top">
              <span className="opt-label">{t("例外")}</span>
              <div style={{ flex: 1, minWidth: 0 }}>
                {form.exceptions.children.length > 0 ? (
                  <GroupEditor
                    node={form.exceptions}
                    onChange={(exceptions) => set({ exceptions: { ...exceptions, mode: "any" } })}
                    fields={targetFields}
                    fieldMap={fieldMap}
                    results={results}
                    modes={false}
                  />
                ) : (
                  <button className="add-btn" onClick={() => set({ exceptions: { ...form.exceptions, children: [newTest(fieldMap)] } })}>
                    <Plus size={14} /> {t("例外")}
                  </button>
                )}
              </div>
            </div>
          </div>
        )}
      </section>

      {error && showErrors && (
        <div className="banner error">
          <TriangleAlert size={16} />
          <span className="selectable">{error}</span>
        </div>
      )}
      {inspecting && testPath && <InspectDialog path={testPath} onClose={() => setInspecting(false)} />}
    </Modal>
  );
}
