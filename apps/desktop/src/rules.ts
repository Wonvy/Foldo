// 规则的编辑模型（条件树）、与配置的互相转换、以及把规则翻译成一句话。
import { baseName, type ActionDef, type ConditionDef, type DestDef, type FieldDef, type FieldType, type RuleDef } from "./api";

const list = <T,>(v: T | T[]): T[] => (Array.isArray(v) ? v : [v]);

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
    { op: "contains", label: "包含" },
    { op: "eq", label: "等于" },
    { op: "ne", label: "不等于" },
    { op: "not_contains", label: "不包含" },
    { op: "starts_with", label: "开头是" },
    { op: "ends_with", label: "结尾是" },
    { op: "in", label: "是其中之一" },
    { op: "not_in", label: "不是其中之一" },
    { op: "glob", label: "符合通配符" },
    { op: "matches", label: "匹配正则" },
    { op: "empty", label: "为空" },
    { op: "exists", label: "有这个属性" },
  ],
  enum: [
    { op: "eq", label: "是" },
    { op: "ne", label: "不是" },
    { op: "in", label: "是其中之一" },
    { op: "not_in", label: "不是其中之一" },
    { op: "exists", label: "有这个属性" },
  ],
  number: [
    { op: "gte", label: "≥" },
    { op: "gt", label: ">" },
    { op: "lte", label: "≤" },
    { op: "lt", label: "<" },
    { op: "eq", label: "=" },
    { op: "ne", label: "≠" },
    { op: "between", label: "介于" },
    { op: "exists", label: "有这个属性" },
  ],
  size: [
    { op: "gt", label: "大于" },
    { op: "lt", label: "小于" },
    { op: "gte", label: "≥" },
    { op: "lte", label: "≤" },
    { op: "between", label: "介于" },
    { op: "eq", label: "等于" },
  ],
  date: [
    { op: "older_than", label: "早于…之前" },
    { op: "newer_than", label: "在最近…之内" },
    { op: "within", label: "在时间段" },
    { op: "before", label: "早于日期" },
    { op: "after", label: "晚于日期" },
    { op: "on", label: "在这一天" },
    { op: "between", label: "在日期之间" },
    { op: "exists", label: "有这个属性" },
  ],
  bool: [
    { op: "eq", label: "是" },
    { op: "exists", label: "有这个属性" },
  ],
};

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
  { value: "today", label: "今天" },
  { value: "yesterday", label: "昨天" },
  { value: "this_week", label: "本周" },
  { value: "last_week", label: "上周" },
  { value: "this_month", label: "本月" },
  { value: "last_month", label: "上个月" },
  { value: "this_year", label: "今年" },
  { value: "last_year", label: "去年" },
];

const today = () => new Date().toISOString().slice(0, 10);

/** 切换字段或运算符时给出一个合理的默认值。 */
export function defaultValue(f: FieldDef | undefined, op: string): unknown {
  const ty = f?.type ?? "text";
  if (op === "exists" || op === "empty") return true;
  if (ty === "bool") return true;
  if (ty === "enum") return op === "in" || op === "not_in" ? [f?.options[0] ?? ""] : (f?.options[0] ?? "");
  if (ty === "text") return op === "in" || op === "not_in" ? [] : op === "glob" ? "*" : "";
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

export type ActionKind = "move" | "copy" | "rename" | "trash" | "keep";
export type ActionRow = { kind: ActionKind; to: string; onConflict: "rename" | "skip" };

function rowFromAction(a: ActionDef): ActionRow {
  if (a === "trash" || a === "keep") return { kind: a, to: "", onConflict: "rename" };
  const [kind, dest] = "move" in a ? (["move", a.move] as const) : "copy" in a ? (["copy", a.copy] as const) : (["rename", a.rename] as const);
  return typeof dest === "string"
    ? { kind, to: dest, onConflict: "rename" }
    : { kind, to: dest.to, onConflict: dest.on_conflict === "skip" ? "skip" : "rename" };
}

// ---------- 规则表单 ----------

export type RuleForm = {
  name: string;
  enabled: boolean;
  scope: "all" | "custom";
  locations: string[];
  recursive: boolean;
  includeHidden: boolean;
  root: GroupNode;
  /** 例外：命中任意一条就不处理（mode 固定为 any）。 */
  exceptions: GroupNode;
  actions: ActionRow[];
  /** 包含表单表达不了的写法（如覆盖已有文件），只能到源码里改。 */
  advanced: boolean;
};

export function emptyForm(fields: FieldMap): RuleForm {
  return {
    name: "",
    enabled: true,
    scope: "all",
    locations: [],
    recursive: false,
    includeHidden: false,
    root: group("all", [newTest(fields, "category")]),
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
    scope: rule.locations ? "custom" : "all",
    locations: rule.locations ? list(rule.locations) : [],
    recursive: !!rule.recursive,
    includeHidden: !!rule.include_hidden,
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
  if (!fields.has(n.field)) return `未知字段「${n.field}」`;
  if (empty(n.value) && n.op !== "empty") return `请填写“${label}”的值`;
  return null;
}

/** 表单 → 配置里的规则；有问题时返回说明。 */
export function formToRule(f: RuleForm, fields: FieldMap): { rule: RuleDef } | { error: string } {
  const name = f.name.trim();
  if (!name) return { error: "请填写规则名称" };
  if (f.scope === "custom" && f.locations.length === 0) return { error: "请至少选择一个生效的文件夹" };
  const e = checkValues(f.root, fields) ?? checkValues(f.exceptions, fields);
  if (e) return { error: e };
  if (f.actions.length === 0) return { error: "请至少添加一个动作" };
  const actions: ActionDef[] = [];
  for (const [i, a] of f.actions.entries()) {
    if (a.kind === "trash" || a.kind === "keep") {
      if (i !== f.actions.length - 1) return { error: "“移到回收站”和“保持原位”只能放在最后" };
      if (a.kind === "keep" && f.actions.length > 1) return { error: "“保持原位”不能和其他动作一起使用" };
      actions.push(a.kind);
      continue;
    }
    const to = a.to.trim();
    if (!to) return { error: a.kind === "rename" ? "请填写新文件名" : "请选择目标文件夹" };
    const dest: DestDef = a.onConflict === "skip" ? { to, on_conflict: "skip" } : to;
    actions.push(a.kind === "move" ? { move: dest } : a.kind === "copy" ? { copy: dest } : { rename: dest });
  }
  const rule: RuleDef = { name, actions };
  if (f.root.children.length > 0 || f.root.not) rule.when = nodeToDef(f.root);
  if (f.exceptions.children.length) rule.except = f.exceptions.children.map(nodeToDef);
  if (!f.enabled) rule.enabled = false;
  if (f.scope === "custom") rule.locations = f.locations;
  if (f.recursive) rule.recursive = true;
  if (f.includeHidden) rule.include_hidden = true;
  return { rule };
}

// ---------- 一句话描述 ----------

function valueText(f: FieldDef | undefined, op: string, v: unknown): string {
  if (op === "exists") return v === false ? "没有" : "有";
  if (op === "empty") return v === false ? "不为空" : "为空";
  if (op === "within") return PERIODS.find((p) => p.value === v)?.label ?? String(v);
  if (op === "older_than" || op === "newer_than") {
    const units: Record<string, string> = { s: "秒", m: "分钟", h: "小时", d: "天", w: "周", mo: "个月", y: "年" };
    return String(v).replace(/(\d+)\s*([a-z]+)/gi, (_, n, u) => `${n} ${units[u.toLowerCase()] ?? u}`);
  }
  if (f?.type === "bool") return v === false ? "否" : "是";
  if (Array.isArray(v)) return op === "between" ? `${v[0]} ~ ${v[1]}` : v.join(" / ");
  const s = String(v);
  return f?.unit && f.type === "number" ? `${s} ${f.unit}` : s;
}

export function opLabel(f: FieldDef | undefined, op: string): string {
  return OPS[f?.type ?? "text"].find((o) => o.op === op)?.label ?? op;
}

export function describeNode(n: CondNode, fields: FieldMap, top = true): string {
  let s: string;
  if (n.kind === "group") {
    const parts = n.children.map((c) => describeNode(c, fields, false));
    if (parts.length === 0) s = "所有文件";
    else if (n.mode === "none") s = `都不是（${parts.join("、")}）`;
    else {
      s = parts.join(n.mode === "all" ? " 且 " : " 或 ");
      if (!top && parts.length > 1) s = `（${s}）`;
    }
  } else {
    const f = fields.get(n.field);
    const label = f?.label ?? n.field;
    if (n.op === "exists" || n.op === "empty" || f?.type === "bool") s = `${label}${n.op === "exists" ? "：" : " "}${valueText(f, n.op, n.value)}`;
    else if (n.op === "older_than") s = `${label}在 ${valueText(f, n.op, n.value)}之前`;
    else if (n.op === "newer_than") s = `${label}在最近 ${valueText(f, n.op, n.value)}内`;
    else s = `${label} ${opLabel(f, n.op)} ${valueText(f, n.op, n.value)}`;
  }
  return n.not ? `不满足（${s}）` : s;
}

export function describeConditions(rule: RuleDef, fields: FieldMap): string {
  return describeNode(ruleToForm(rule, fields).root, fields);
}

export function describeExceptions(rule: RuleDef, fields: FieldMap): string | null {
  const ex = ruleToForm(rule, fields).exceptions;
  return ex.children.length ? ex.children.map((c) => describeNode(c, fields, false)).join("、") : null;
}

function destText(d: DestDef): string {
  return tilde(typeof d === "string" ? d : d.to);
}

export function describeAction(a: ActionDef): string {
  if (a === "trash") return "移到回收站";
  if (a === "keep") return "保持原位";
  if ("move" in a) return `移到 ${destText(a.move)}`;
  if ("copy" in a) return `复制到 ${destText(a.copy)}`;
  return `改名为 ${destText(a.rename)}`;
}

export function describeActions(rule: RuleDef): string {
  return rule.actions.map(describeAction).join("，再");
}

export function describeScope(rule: RuleDef): string {
  if (!rule.locations) return "所有整理文件夹";
  return list(rule.locations).map(baseName).join("、");
}
