// 新建 / 编辑规则：“如果 … 就 …”。规则完全由用户填写，“用文件测试”只用来检查。
import {
  ArrowRight,
  ChevronDown,
  ChevronRight,
  CircleCheck,
  CircleX,
  Code,
  Copy,
  File as FileIcon2,
  Folder as FolderIcon,
  FolderOpen,
  Layers,
  Link2,
  Lock,
  Package,
  PackageOpen,
  PenLine,
  Plus,
  ScanSearch,
  Trash,
  TriangleAlert,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useMemo, useRef, useState } from "react";
import { api, baseName, errorText, type RuleDef, type RuleTarget, type TestView } from "../api";
import { DEFAULT_FIELD, emptyForm, fieldFits, formToRule, group, newTest, renameInFolders, ruleToForm, tilde, type CondNode, type FieldMap, type ActionKind, type ActionRow, type DestKind, type RuleForm, ACTION_DEFAULT_TO } from "../rules";
import { pickFile, pickFolder, useStore } from "../store";
import GroupEditor, { mapResults, type ResultMap } from "./ConditionTree";
import InspectDialog from "./InspectDialog";
import Popover from "./Popover";
import { Modal, Segmented, Spinner, Switch, useMenu } from "./ui";
import VarMenu from "./VarMenu";
import { t } from "../i18n";

const ACTIONS: { kind: ActionKind; label: string; icon: LucideIcon }[] = [
  { kind: "move", label: t("移动"), icon: ArrowRight },
  { kind: "copy", label: t("复制"), icon: Copy },
  { kind: "rename", label: t("改名"), icon: PenLine },
  { kind: "extract", label: t("解压"), icon: PackageOpen },
  { kind: "compress", label: t("压缩"), icon: Package },
  { kind: "shortcut", label: t("快捷方式"), icon: Link2 },
  { kind: "trash", label: t("移到回收站"), icon: Trash },
  { kind: "keep", label: t("保持原位"), icon: Lock },
];

const PLACEHOLDER: Record<DestKind, string> = {
  move: t("目标文件夹，如 照片/{modified:%Y}/"),
  copy: t("目标文件夹，如 ~/备份/"),
  rename: t("新文件名，如 {exif.taken:%Y%m%d}_{name}"),
  extract: t("解压到的文件夹，相对压缩包所在位置，如 {stem}/"),
  compress: t("压缩包名称，相对文件所在位置，如 {stem}.zip"),
  shortcut: t("放快捷方式的文件夹，如 ~/Desktop/常用/"),
};

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
  const initial = useMemo(() => (ready ? (original ? ruleToForm(original, fieldMap) : emptyForm(fieldMap)) : null), [original, ready, fieldMap]);
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
          const names = [...new Set([...p.ops.map((o) => o.origin_name), ...p.skipped.filter((s) => s.rule === "__试运行__").map((s) => s.name)])];
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

  const moreSummary = [
    form.exceptions.children.length > 0 && t("{0} 个例外", [form.exceptions.children.length]),
  ]
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
            <button className="icon-btn" title={t("删除规则")} aria-label={t("删除规则")} onClick={remove} style={{ color: "var(--danger)" }}>
              <Trash size={16} />
            </button>
          )}
          <button className="btn" onClick={onClose}>
            {t("取消")}
          </button>
          <button className="btn primary" disabled={saving} onClick={submit}>
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
                { value: "file", label: <><FileIcon2 size={14} /> {t("文件")}</> },
                { value: "folder", label: <><FolderIcon size={14} /> {t("文件夹")}</> },
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

// ---------- 底栏的两个小标签 ----------

function MatchChip({ preview, invalid, error }: { preview: { count: number; names: string[] } | null; invalid: boolean; error: string | null }) {
  const ref = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  if (invalid) return <span className="foot-chip muted">{t("命中 —")}</span>;
  if (error) return <span className="foot-chip" title={error}><TriangleAlert size={13} /> {t("预览失败")}</span>;
  if (!preview)
    return (
      <span className="foot-chip muted">
        <Spinner size={13} />
      </span>
    );
  return (
    <>
      <button ref={ref} className={`foot-chip ${preview.count ? "accent" : ""}`} title={t("整理文件夹里现在符合的文件")} onClick={() => preview.count && setOpen(!open)}>
        <Layers size={14} /> {t("命中")} {preview.count}
      </button>
      {open && (
        <Popover anchor={ref} onClose={() => setOpen(false)} width={280}>
          <div className="pick-list">
            {preview.names.map((n) => (
              <div key={n} className="pick-item ellipsis">
                {n}
              </div>
            ))}
            {preview.count > preview.names.length && <div className="pick-item muted">{t("还有 {0} 个…", [preview.count - preview.names.length])}</div>}
          </div>
        </Popover>
      )}
    </>
  );
}

function TestChip({
  path,
  view,
  error,
  onPick,
  onInspect,
  onClear,
}: {
  path: string | null;
  view: TestView | null;
  error: string | null;
  onPick: () => void;
  onInspect: () => void;
  onClear: () => void;
}) {
  if (!path) {
    return (
      <button className="foot-chip" title={t("选一个文件，看每个条件是否成立、会放到哪里")} onClick={onPick}>
        <ScanSearch size={14} /> {t("测试（可拖入文件）")}
      </button>
    );
  }
  const ok = !!view?.destination;
  return (
    <span className={`foot-chip test ${view ? (ok ? "ok" : "no") : ""}`}>
      {error ? <TriangleAlert size={14} /> : !view ? <Spinner size={13} /> : ok ? <CircleCheck size={14} /> : <CircleX size={14} />}
      <button className="link-plain ellipsis" title={t("{0}\n点击查看属性", [path])} onClick={onInspect}>
        {baseName(path)}
      </button>
      {error && <span className="ellipsis" role="alert" title={error} style={{ maxWidth: 220 }}>{error}</span>}
      {!error && view && (
        <span className="ellipsis" title={view.destination ?? view.reason ?? ""} style={{ maxWidth: 220 }}>
          {ok ? `→ ${tilde(view.destination!)}` : (view.reason ?? t("不满足"))}
        </span>
      )}
      <button className="icon-btn tiny" title={t("换一个文件")} onClick={onPick}>
        <ScanSearch size={13} />
      </button>
      <button className="icon-btn tiny" title={t("结束测试")} onClick={onClear}>
        <X size={13} />
      </button>
    </span>
  );
}

// ---------- 动作 ----------

function ActionList({ rows, folder, onChange }: { rows: ActionRow[]; folder: boolean; onChange: (rows: ActionRow[]) => void }) {
  const update = (i: number, patch: Partial<ActionRow>) => onChange(rows.map((r, j) => (j === i ? { ...r, ...patch } : r)));
  const menu = useMenu();
  const defaultTo = (k: ActionKind) => (k === "trash" || k === "keep" ? undefined : ACTION_DEFAULT_TO[k]);
  const pickKind = (el: HTMLElement, i: number) =>
    menu.open(
      el,
      ACTIONS.filter((a) => !(folder && a.kind === "extract")).map(({ kind, label, icon: Icon }) => ({
        label,
        icon: <Icon size={15} />,
        onClick: () => {
          // 原来的目标为空或是旧动作的默认值时，换成新动作的默认值
          const old = rows[i];
          const custom = old.to && old.to !== defaultTo(old.kind);
          update(i, { kind, to: custom ? old.to : (defaultTo(kind) ?? "") });
        },
      })),
    );
  return (
    <div className="cond-group">
      {rows.map((a, i) => {
        const cur = ACTIONS.find((x) => x.kind === a.kind) ?? ACTIONS[0];
        return (
        <div key={i} className="action-row">
          <button className="action-kind" onClick={(e) => pickKind(e.currentTarget, i)}>
            <cur.icon size={15} />
            <span>{cur.label}</span>
            <ChevronDown size={13} />
          </button>
          {a.kind !== "trash" && a.kind !== "keep" && (
            <div className="dest">
              {(a.kind === "move" || a.kind === "copy" || a.kind === "shortcut" || a.kind === "extract") && (
                <button
                  className="icon-btn"
                  title={t("选择文件夹")}
                  onClick={async () => {
                    const d = await pickFolder(t("选择目标文件夹"));
                    if (d) update(i, { to: d });
                  }}
                >
                  <FolderOpen size={16} />
                </button>
              )}
              <input
                className="dest-input mono"
                placeholder={PLACEHOLDER[a.kind]}
                value={a.kind === "rename" || a.kind === "compress" ? a.to : tilde(a.to)}
                title={a.to}
                onChange={(e) => update(i, { to: e.target.value })}
              />
              <VarMenu onInsert={(v) => update(i, { to: a.to + v })} />
              <select
                className="pill-select"
                value={a.onConflict}
                title={t("目标位置已有同名文件时")}
                onChange={(e) => update(i, { onConflict: e.target.value as "rename" | "skip" })}
              >
                <option value="rename">{t("同名保留两份")}</option>
                <option value="skip">{t("同名跳过")}</option>
              </select>
            </div>
          )}
          {rows.length > 1 && (
            <button className="icon-btn" aria-label={t("删除这一步")} title={t("删除这一步")} onClick={() => onChange(rows.filter((_, j) => j !== i))}>
              <X size={15} />
            </button>
          )}
        </div>
        );
      })}
      {menu.node}
      <div className="add-row">
        <button className="add-btn" onClick={() => onChange([...rows, { kind: "move", to: "", onConflict: "rename" }])}>
          <Plus size={14} /> {t("再加一步")}
        </button>
      </div>
    </div>
  );
}

