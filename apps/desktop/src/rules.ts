// 规则的编辑模型（条件树）、与配置的互相转换、以及把规则翻译成一句话。
import { type ActionDef, type ConditionDef, type DestDef, type FieldDef, type FieldType, type FolderDef, type RuleDef, type RuleFile, type RuleTarget } from "./api";
import { t, tv } from "./i18n";

// ---------- 路径显示 ----------

let home = "";
/** 由 store 在读取配置后设置，用于把路径显示成 ~/… */
export function setHome(h: string) {
  home = h.replace(/[\\/]+$/, "");
}

/** `/Users/me/Pictures` → `~/Pictures` */
export function tilde(p: string): string {
  if (home && (p === home || p.startsWith(home + "/") || p.startsWith(home + "\\"))) return "~" + p.slice(home.length);
  return p;
}

export function formatBytes(n: number): string {
  if (n < 1000) return `${n} B`;
  const units = ["KB", "MB", "GB", "TB"];
  let v = n;
  let i = -1;
  do {
    v /= 1000;
    i++;
  } while (v >= 1000 && i < units.length - 1);
  return `${v >= 10 ? Math.round(v) : v.toFixed(1)} ${units[i]}`;
}

// ---------- 运算符 ----------

export const OPS: Record<FieldType, { op: string; label: string }[]> = {
  text: [
    { op: "contains", label: t("包含") },
    { op: "eq", label: t("等于") },
    { op: "ne", label: t("不等于") },
    { op: "not_contains", label: t("不包含") },
    { op: "starts_with", label: t("开头是") },
    { op: "ends_with", label: t("结尾是") },
    { op: "in", label: t("是其中之一") },
    { op: "not_in", label: t("不是其中之一") },
    { op: "glob", label: t("符合通配符") },
    { op: "matches", label: t("匹配正则") },
    { op: "empty", label: t("为空") },
    { op: "exists", label: t("有这个属性") },
  ],
  enum: [
    { op: "eq", label: t("是") },
    { op: "ne", label: t("不是") },
    { op: "in", label: t("是其中之一") },
    { op: "not_in", label: t("不是其中之一") },
    { op: "exists", label: t("有这个属性") },
  ],
  number: [
    { op: "gte", label: "≥" },
    { op: "gt", label: ">" },
    { op: "lte", label: "≤" },
    { op: "lt", label: "<" },
    { op: "eq", label: "=" },
    { op: "ne", label: "≠" },
    { op: "between", label: t("介于") },
    { op: "exists", label: t("有这个属性") },
  ],
  size: [
    { op: "gt", label: t("大于") },
    { op: "lt", label: t("小于") },
    { op: "gte", label: "≥" },
    { op: "lte", label: "≤" },
    { op: "between", label: t("介于") },
    { op: "eq", label: t("等于") },
  ],
  date: [
    { op: "older_than", label: t("早于…之前") },
    { op: "newer_than", label: t("在最近…之内") },
    { op: "within", label: t("在时间段") },
    { op: "before", label: t("早于日期") },
    { op: "after", label: t("晚于日期") },
    { op: "on", label: t("在这一天") },
    { op: "between", label: t("在日期之间") },
    { op: "exists", label: t("有这个属性") },
  ],
  bool: [
    { op: "eq", label: t("是") },
    { op: "exists", label: t("有这个属性") },
  ],
  // 一组名称：任意一项满足即可；“没有…”表示一项都不满足
  list: [
    { op: "contains", label: t("有项目包含") },
    { op: "eq", label: t("有项目名为") },
    { op: "glob", label: t("有项目符合通配符") },
    { op: "matches", label: t("有项目匹配正则") },
    { op: "not_contains", label: t("没有项目包含") },
    { op: "ne", label: t("没有项目名为") },
    { op: "empty", label: t("为空") },
  ],
};

/** 字段能否用于这类处理对象。 */
export const fieldFits = (f: FieldDef | undefined, target: RuleTarget) => !!f && (f.applies === "both" || f.applies === target);

/** 新条件默认用的字段。 */
export const DEFAULT_FIELD: Record<RuleTarget, string> = { file: "category", folder: "folder.content" };

const ALIASES: Record<string, string> = {
  "=": "eq",
  "==": "eq",
  is: "eq",
  equals: "eq",
  "!=": "ne",
  "≠": "ne",
  "<>": "ne",
  is_not: "ne",
  ">": "gt",
  ">=": "gte",
  "≥": "gte",
  "<": "lt",
  "<=": "lte",
  "≤": "lte",
  regex: "matches",
  is_empty: "empty",
};
const canon = (op: string) => ALIASES[op] ?? op;

export const PERIODS: { value: string; label: string }[] = [
  { value: "today", label: t("今天") },
  { value: "yesterday", label: t("昨天") },
  { value: "this_week", label: t("本周") },
  { value: "last_week", label: t("上周") },
  { value: "this_month", label: t("本月") },
  { value: "last_month", label: t("上个月") },
  { value: "this_year", label: t("今年") },
  { value: "last_year", label: t("去年") },
];

const today = () => new Date().toISOString().slice(0, 10);

/** 切换字段或运算符时给出一个合理的默认值。 */
export function defaultValue(f: FieldDef | undefined, op: string): unknown {
  const ty = f?.type ?? "text";
  if (op === "exists" || op === "empty") return true;
  if (ty === "bool") return true;
  if (ty === "enum") return op === "in" || op === "not_in" ? [f?.options[0] ?? ""] : (f?.options[0] ?? "");
  if (ty === "text" || ty === "list") return op === "in" || op === "not_in" ? [] : op === "glob" ? "*" : "";
  if (ty === "size") return op === "between" ? ["1MB", "10MB"] : "10MB";
  if (ty === "number") return op === "between" ? [0, 100] : 0;
  // date
  if (op === "older_than" || op === "newer_than") return "30d";
  if (op === "within") return "this_month";
  if (op === "between") return [today(), today()];
  return today();
}

// ---------- 条件树 ----------

export type GroupMode = "all" | "any" | "none";
export type TestNode = { id: string; kind: "test"; field: string; op: string; value: unknown; cs: boolean; not: boolean };
export type GroupNode = { id: string; kind: "group"; mode: GroupMode; not: boolean; children: CondNode[] };
export type CondNode = TestNode | GroupNode;
export type FieldMap = Map<string, FieldDef>;

let seq = 0;
export const nid = () => `n${++seq}`;

export const group = (mode: GroupMode, children: CondNode[] = []): GroupNode => ({ id: nid(), kind: "group", mode, not: false, children });

export function newTest(fields: FieldMap, key = "name"): TestNode {
  const f = fields.get(key);
  const op = OPS[f?.type ?? "text"][0].op;
  return { id: nid(), kind: "test", field: key, op, value: defaultValue(f, op), cs: false, not: false };
}

/** 配置里的一个条件 → 编辑节点。一个字段写了多个运算符时拆成“全部满足”的小组，含义不变。 */
export function defToNode(def: ConditionDef, fields: FieldMap): CondNode {
  const [key, value] = Object.entries(def)[0] ?? ["name", ""];
  if (key === "all" || key === "any" || key === "none") {
    return group(key, ((value as ConditionDef[]) ?? []).map((d) => defToNode(d, fields)));
  }
  if (key === "not") {
    const inner = defToNode(value as ConditionDef, fields);
    return { ...inner, not: !inner.not };
  }
  const field = key === "extension" ? "ext" : key;
  const f = fields.get(field);
  const leaf = (op: string, v: unknown, cs = false): TestNode => ({ id: nid(), kind: "test", field, op: canon(op), value: v, cs, not: false });
  if (value !== null && typeof value === "object" && !Array.isArray(value)) {
    const { case_sensitive, ...ops } = value as Record<string, unknown>;
    const leaves = Object.entries(ops).map(([op, v]) => leaf(op, v, case_sensitive === true));
    return leaves.length === 1 ? leaves[0] : group("all", leaves);
  }
  if (Array.isArray(value)) return leaf("in", value);
  const op = f?.type === "text" && typeof value === "string" && /[*?]/.test(value) ? "glob" : f?.type === "date" ? "on" : "eq";
  return leaf(op, value);
}

/** 编辑节点 → 配置里的一个条件。 */
export function nodeToDef(n: CondNode): ConditionDef {
  let d: ConditionDef;
  if (n.kind === "group") {
    d = { [n.mode]: n.children.map(nodeToDef) };
  } else {
    const body: Record<string, unknown> = { [n.op]: n.value };
    if (n.cs) body.case_sensitive = true;
    d = { [n.field]: body };
  }
  return n.not ? { not: d } : d;
}

// ---------- 动作 ----------

export type ActionKind = "move" | "copy" | "rename" | "extract" | "compress" | "shortcut" | "trash" | "keep";
/** 带目标路径的动作。 */
export type DestKind = Exclude<ActionKind, "trash" | "keep">;
const DEST_KINDS: DestKind[] = ["move", "copy", "rename", "extract", "compress", "shortcut"];

/** 动作的动词（“移到 …”）。 */
export const ACTION_VERB: Record<DestKind, string> = {
  move: t("移到"),
  copy: t("复制到"),
  rename: t("改名为"),
  extract: t("解压到"),
  compress: t("压缩为"),
  shortcut: t("快捷方式放到"),
};

/** 切换成这种动作时的默认目标。 */
export const ACTION_DEFAULT_TO: Partial<Record<DestKind, string>> = {
  extract: "{stem}/",
  compress: "{stem}.zip",
};
export type ActionRow = { kind: ActionKind; to: string; onConflict: "rename" | "skip" };

function rowFromAction(a: ActionDef): ActionRow {
  if (a === "trash" || a === "keep") return { kind: a, to: "", onConflict: "rename" };
  const kind = DEST_KINDS.find((k) => k in a) ?? "move";
  const dest = (a as Record<string, DestDef>)[kind];
  return typeof dest === "string"
    ? { kind, to: dest, onConflict: "rename" }
    : { kind, to: dest.to, onConflict: dest.on_conflict === "skip" ? "skip" : "rename" };
}

// ---------- 规则表单 ----------

export type RuleForm = {
  name: string;
  enabled: boolean;
  /** 处理文件还是文件夹 */
  target: RuleTarget;
  root: GroupNode;
  /** 例外：命中任意一条就不处理（mode 固定为 any）。 */
  exceptions: GroupNode;
  actions: ActionRow[];
  /** 包含表单表达不了的写法（如覆盖已有文件），只能到源码里改。 */
  advanced: boolean;
};

export function emptyForm(fields: FieldMap, target: RuleTarget = "file"): RuleForm {
  return {
    name: "",
    enabled: true,
    target,
    root: group("all", [newTest(fields, DEFAULT_FIELD[target])]),
    exceptions: group("any"),
    actions: [{ kind: "move", to: "", onConflict: "rename" }],
    advanced: false,
  };
}

/** 配置里的规则 → 表单（旧写法 conditions + match 与 when 合并成一棵树）。 */
export function ruleToForm(rule: RuleDef, fields: FieldMap): RuleForm {
  const parts: CondNode[] = [];
  const conds = (rule.conditions ?? []).map((c) => defToNode(c, fields));
  if (conds.length) parts.push(group(rule.match === "any" ? "any" : "all", conds));
  if (rule.when) parts.push(defToNode(rule.when, fields));
  let root: GroupNode;
  if (parts.length === 0) root = group("all");
  else if (parts.length === 1 && parts[0].kind === "group" && !parts[0].not) root = parts[0];
  else root = group("all", parts);
  const advanced = rule.actions.some(
    (a) => typeof a === "object" && typeof Object.values(a)[0] === "object" && (Object.values(a)[0] as { on_conflict?: string }).on_conflict === "overwrite",
  );
  return {
    name: rule.name,
    enabled: rule.enabled !== false,
    target: rule.target === "folder" ? "folder" : "file",
    root,
    exceptions: group("any", (rule.except ?? []).map((c) => defToNode(c, fields))),
    actions: rule.actions.map(rowFromAction),
    advanced,
  };
}

function checkValues(n: CondNode, fields: FieldMap): string | null {
  if (n.kind === "group") {
    for (const c of n.children) {
      const e = checkValues(c, fields);
      if (e) return e;
    }
    return null;
  }
  const label = fields.get(n.field)?.label ?? n.field;
  const empty = (v: unknown) => v === "" || v === null || v === undefined || (Array.isArray(v) && (v.length === 0 || v.some((x) => x === "")));
  if (!fields.has(n.field)) return t("未知字段「{0}」", [n.field]);
  if (empty(n.value) && n.op !== "empty") return t("请填写“{0}”的值", [label]);
  return null;
}

/** 表单 → 配置里的规则；有问题时返回说明。 */
export function formToRule(f: RuleForm, fields: FieldMap): { rule: RuleDef } | { error: string } {
  const name = f.name.trim();
  if (!name) return { error: t("请填写规则名称") };
  const e = checkValues(f.root, fields) ?? checkValues(f.exceptions, fields);
  if (e) return { error: e };
  if (f.actions.length === 0) return { error: t("请至少添加一个动作") };
  const actions: ActionDef[] = [];
  for (const [i, a] of f.actions.entries()) {
    if (a.kind === "trash" || a.kind === "keep") {
      if (i !== f.actions.length - 1) return { error: t("“移到回收站”和“保持原位”只能放在最后") };
      if (a.kind === "keep" && f.actions.length > 1) return { error: t("“保持原位”不能和其他动作一起使用") };
      actions.push(a.kind);
      continue;
    }
    const to = a.to.trim();
    if (!to) return { error: a.kind === "rename" ? t("请填写新文件名") : a.kind === "compress" ? t("请填写压缩包名称") : t("请选择目标文件夹") };
    const dest: DestDef = a.onConflict === "skip" ? { to, on_conflict: "skip" } : to;
    actions.push({ [a.kind]: dest } as ActionDef);
  }
  const rule: RuleDef = { name, actions };
  if (f.root.children.length > 0 || f.root.not) rule.when = nodeToDef(f.root);
  if (f.exceptions.children.length) rule.except = f.exceptions.children.map(nodeToDef);
  if (!f.enabled) rule.enabled = false;
  if (f.target === "folder") rule.target = "folder";
  return { rule };
}

// ---------- 一句话描述 ----------

function valueText(f: FieldDef | undefined, op: string, v: unknown): string {
  if (op === "exists") return v === false ? t("没有") : t("有");
  if (op === "empty") return v === false ? t("不为空") : t("为空");
  if (op === "within") return PERIODS.find((p) => p.value === v)?.label ?? String(v);
  if (op === "older_than" || op === "newer_than") {
    const units: Record<string, string> = { s: t("秒"), m: t("分钟"), h: t("小时"), d: t("天"), w: t("周"), mo: t("个月"), y: t("年") };
    return String(v).replace(/(\d+)\s*([a-z]+)/gi, (_, n, u) => `${n} ${units[u.toLowerCase()] ?? u}`);
  }
  if (f?.type === "bool") return v === false ? tv("否") : tv("是");
  const show = (x: unknown) => (f?.type === "enum" ? t(String(x)) : String(x));
  if (Array.isArray(v)) return op === "between" ? `${v[0]} ~ ${v[1]}` : v.map(show).join(" / ");
  const s = show(v);
  return f?.unit && f.type === "number" ? `${s} ${f.unit}` : s;
}

export function opLabel(f: FieldDef | undefined, op: string): string {
  return OPS[f?.type ?? "text"].find((o) => o.op === op)?.label ?? op;
}

export function describeNode(n: CondNode, fields: FieldMap, top = true): string {
  let s: string;
  if (n.kind === "group") {
    const parts = n.children.map((c) => describeNode(c, fields, false));
    if (parts.length === 0) s = t("所有文件");
    else if (n.mode === "none") s = t("都不是（{0}）", [parts.join("、")]);
    else {
      s = parts.join(n.mode === "all" ? t(" 且 ") : t(" 或 "));
      if (!top && parts.length > 1) s = t("（{0}）", [s]);
    }
  } else {
    const f = fields.get(n.field);
    const label = f?.label ?? n.field;
    if (n.op === "exists" || n.op === "empty" || f?.type === "bool") s = `${label}${n.op === "exists" ? t("：") : " "}${valueText(f, n.op, n.value)}`;
    else if (n.op === "older_than") s = t("{0}在 {1}之前", [label, valueText(f, n.op, n.value)]);
    else if (n.op === "newer_than") s = t("{0}在最近 {1}内", [label, valueText(f, n.op, n.value)]);
    else s = `${label} ${opLabel(f, n.op)} ${valueText(f, n.op, n.value)}`;
  }
  return n.not ? t("不满足（{0}）", [s]) : s;
}

export function describeConditions(rule: RuleDef, fields: FieldMap): string {
  return describeNode(ruleToForm(rule, fields).root, fields);
}

export function describeExceptions(rule: RuleDef, fields: FieldMap): string | null {
  const ex = ruleToForm(rule, fields).exceptions;
  return ex.children.length ? ex.children.map((c) => describeNode(c, fields, false)).join(t("、")) : null;
}

function destText(d: DestDef): string {
  return tilde(typeof d === "string" ? d : d.to);
}

export function describeAction(a: ActionDef): string {
  if (a === "trash") return t("移到回收站");
  if (a === "keep") return t("保持原位");
  const kind = DEST_KINDS.find((k) => k in a) ?? "move";
  return `${ACTION_VERB[kind]} ${destText((a as Record<string, DestDef>)[kind])}`;
}

export function describeActions(rule: RuleDef): string {
  return rule.actions.map(describeAction).join(t("，再"));
}

// ---------- 文件夹使用的规则 ----------

/** 去掉文件夹里引用的已不存在的规则（删除规则后）。 */
export function pruneFolderRules(file: RuleFile): RuleFile {
  const names = new Set((file.rules ?? []).map((r) => r.name));
  const folders = (file.folders ?? []).map((f) => ({ ...f, rules: (f.rules ?? []).filter((n) => names.has(n)) }));
  return { ...file, folders };
}

/** 规则改名后同步文件夹里的引用。 */
export function renameInFolders(file: RuleFile, from: string, to: string): RuleFile {
  if (from === to) return file;
  const folders = (file.folders ?? []).map((f) => ({ ...f, rules: (f.rules ?? []).map((n) => (n === from ? to : n)) }));
  return { ...file, folders };
}

/** 使用某条规则的文件夹。 */
export function foldersUsing(file: RuleFile, name: string): FolderDef[] {
  return (file.folders ?? []).filter((f) => f.rules?.includes(name));
}

// ---------- 列表里的标签摘要 ----------

export type CondChip = { kind: "test" | "group"; group: string; text: string; title: string; not: boolean };
export type ActionChip = { kind: ActionKind; text: string; title: string };

/** 长路径只保留最后两级：~/Documents/工作/2026/发票 → …/2026/发票 */
export function shortPath(p: string): string {
  const full = tilde(p).replace(/[\\/]+$/, "");
  if (full.length <= 26) return full;
  const parts = full.split(/[\\/]/);
  return parts.length > 2 ? `…/${parts.slice(-2).join("/")}` : full;
}

const MODE_LABEL: Record<GroupMode, string> = { all: t("全部"), any: t("任一"), none: t("都不") };

export function ruleChips(rule: RuleDef, fields: FieldMap): { mode: GroupMode; conds: CondChip[]; actions: ActionChip[]; exceptions: number } {
  const form = ruleToForm(rule, fields);
  const conds: CondChip[] = form.root.children.map((n) => {
    if (n.kind === "test") {
      const text = describeNode({ ...n, not: false }, fields);
      return { kind: "test", group: fields.get(n.field)?.group ?? t("基本"), text, title: describeNode(n, fields), not: n.not };
    }
    return { kind: "group", group: "", text: t("{0}满足 {1} 项", [MODE_LABEL[n.mode], n.children.length]), title: describeNode(n, fields), not: n.not };
  });
  const actions: ActionChip[] = form.actions.map((a) => {
    if (a.kind === "trash") return { kind: a.kind, text: t("回收站"), title: t("移到回收站") };
    if (a.kind === "keep") return { kind: a.kind, text: t("保持原位"), title: t("保持原位") };
    const verb = ACTION_VERB[a.kind];
    const named = a.kind === "rename" || a.kind === "compress" || a.kind === "extract";
    return { kind: a.kind, text: named ? a.to : shortPath(a.to), title: `${verb} ${tilde(a.to)}` };
  });
  return { mode: form.root.mode, conds, actions, exceptions: form.exceptions.children.length };
}
