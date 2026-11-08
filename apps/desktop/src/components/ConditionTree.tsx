// 条件树编辑器：组（全部 / 任一 / 都不）可以任意嵌套，每个条件可以单独取反。
import { Ban, CaseSensitive, CircleCheck, CircleX, Ellipsis, Layers, Plus, Trash } from "lucide-react";
import { useEffect, useState } from "react";
import type { Explain, FieldDef } from "../api";
import { OPS, PERIODS, defaultValue, group, newTest, type CondNode, type FieldMap, type GroupNode, type TestNode } from "../rules";
import FieldPicker from "./FieldPicker";
import { useMenu } from "./ui";
import { t, tv } from "../i18n";

/** 节点 id → 测试结果（用文件测试时显示在每个条件旁边）。 */
export type ResultMap = Map<string, Explain>;

/** 把后端返回的解释树对应到编辑节点上（结构与 nodeToDef 生成的一致）。 */
export function mapResults(n: CondNode, ex: Explain | undefined, out: ResultMap) {
  if (!ex) return;
  out.set(n.id, ex);
  const inner = n.not ? ex.children?.[0] : ex;
  if (n.kind === "group" && inner) n.children.forEach((c, i) => mapResults(c, inner.children?.[i], out));
}

export default function GroupEditor({
  node,
  onChange,
  onRemove,
  fields,
  fieldMap,
  results,
  depth = 0,
  modes = true,
}: {
  node: GroupNode;
  onChange: (n: GroupNode) => void;
  onRemove?: () => void;
  fields: FieldDef[];
  fieldMap: FieldMap;
  results?: ResultMap;
  depth?: number;
  /** 例外列表固定为“任一”，不显示切换。 */
  modes?: boolean;
}) {
  const menu = useMenu();
  const setChild = (i: number, c: CondNode | null) =>
    onChange({ ...node, children: c ? node.children.map((x, j) => (j === i ? c : x)) : node.children.filter((_, j) => j !== i) });
  const showHead = (modes && node.children.length > 1) || depth > 0;

  return (
    <div className={`cond-group ${depth > 0 ? "nested" : ""}`}>
      {showHead && (
        <div className="cond-group-head">
          {depth > 0 && <StatusDot ex={results?.get(node.id)} />}
          {node.not && <span className="not-badge">{t("非")}</span>}
          <select
            className="pill-select"
            value={node.mode}
            onChange={(e) => onChange({ ...node, mode: e.target.value as GroupNode["mode"] })}
          >
            <option value="all">{t("全部满足")}</option>
            <option value="any">{t("任一满足")}</option>
            <option value="none">{t("都不满足")}</option>
          </select>
          <span className="spacer" />
          {depth > 0 && (
            <button
              className="icon-btn row-more"
              aria-label={t("更多")}
              onClick={(e) =>
                menu.open(e.currentTarget, [
                  {
                    label: node.not ? t("取消取反") : t("取反（不满足这一组时命中）"),
                    icon: <Ban size={15} />,
                    onClick: () => onChange({ ...node, not: !node.not }),
                  },
                  "divider",
                  { label: t("删除这一组"), icon: <Trash size={15} />, danger: true, onClick: () => onRemove?.() },
                ])
              }
            >
              <Ellipsis size={16} />
            </button>
          )}
        </div>
      )}
      {node.children.map((c, i) =>
        c.kind === "group" ? (
          <GroupEditor
            key={c.id}
            node={c}
            onChange={(n) => setChild(i, n)}
            onRemove={() => setChild(i, null)}
            fields={fields}
            fieldMap={fieldMap}
            results={results}
            depth={depth + 1}
          />
        ) : (
          <TestRow
            key={c.id}
            node={c}
            onChange={(n) => setChild(i, n)}
            onRemove={() => setChild(i, null)}
            fields={fields}
            fieldMap={fieldMap}
            result={results?.get(c.id)}
          />
        ),
      )}
      <div className="add-row">
        <button className="add-btn" onClick={() => onChange({ ...node, children: [...node.children, newTest(fieldMap)] })}>
          <Plus size={14} /> {t("条件")}
        </button>
        <button
          className="add-btn"
          title={t("添加一组条件，组内可以选“任一满足”等")}
          onClick={() =>
            onChange({ ...node, children: [...node.children, group(node.mode === "all" ? "any" : "all", [newTest(fieldMap)])] })
          }
        >
          <Layers size={14} /> {t("条件组")}
        </button>
      </div>
      {menu.node}
    </div>
  );
}

function StatusDot({ ex }: { ex: Explain | undefined }) {
  if (!ex) return null;
  const title = ex.error ?? (ex.ok ? t("这个文件满足") : t("这个文件不满足"));
  return (
    <span className="status-dot" title={title}>
      {ex.ok ? <CircleCheck size={15} color="var(--ok)" /> : <CircleX size={15} color={ex.error ? "var(--danger)" : "var(--muted)"} />}
    </span>
  );
}

function TestRow({
  node,
  onChange,
  onRemove,
  fields,
  fieldMap,
  result,
}: {
  node: TestNode;
  onChange: (n: TestNode) => void;
  onRemove: () => void;
  fields: FieldDef[];
  fieldMap: FieldMap;
  result?: Explain;
}) {
  const menu = useMenu();
  const f = fieldMap.get(node.field);
  const ops = OPS[f?.type ?? "text"];
  const caseable = (f?.type === "text" || f?.type === "list") && !["exists", "empty", "in", "not_in"].includes(node.op);

  const setField = (key: string) => {
    const nf = fieldMap.get(key);
    const nextOps = OPS[nf?.type ?? "text"];
    const op = nextOps.some((o) => o.op === node.op) ? node.op : nextOps[0].op;
    onChange({ ...node, field: key, op, value: defaultValue(nf, op) });
  };
  const setOp = (op: string) => {
    const keep =
      (f?.type === "text" || f?.type === "enum" || f?.type === "list") &&
      typeof node.value === "string" &&
      !["in", "not_in", "exists", "empty"].includes(op) &&
      !["exists", "empty"].includes(node.op);
    onChange({ ...node, op, value: keep ? node.value : defaultValue(f, op) });
  };

  return (
    <div className="cond-row">
      <StatusDot ex={result} />
      {node.not && (
        <button className="not-badge" title={t("已取反，点击取消")} onClick={() => onChange({ ...node, not: false })}>
          {t("非")}
        </button>
      )}
      <FieldPicker value={node.field} fields={fields} onChange={setField} />
      <select className="pill-select" value={node.op} onChange={(e) => setOp(e.target.value)}>
        {ops.map((o) => (
          <option key={o.op} value={o.op}>
            {o.label}
          </option>
        ))}
      </select>
      <ValueInput field={f} op={node.op} value={node.value} onChange={(value) => onChange({ ...node, value })} />
      {node.cs && caseable && (
        <span className="mini-badge" title={t("区分大小写")}>
          Aa
        </span>
      )}
      <button
        className="icon-btn row-more"
        aria-label={t("更多")}
        onClick={(e) =>
          menu.open(e.currentTarget, [
            {
              label: node.not ? t("取消取反") : t("取反（不满足时命中）"),
              icon: <Ban size={15} />,
              onClick: () => onChange({ ...node, not: !node.not }),
            },
            ...(caseable
              ? [
                  {
                    label: node.cs ? t("不区分大小写") : t("区分大小写"),
                    icon: <CaseSensitive size={15} />,
                    onClick: () => onChange({ ...node, cs: !node.cs }),
                  },
                ]
              : []),
            "divider" as const,
            { label: t("删除条件"), icon: <Trash size={15} />, danger: true, onClick: onRemove },
          ])
        }
      >
        <Ellipsis size={16} />
      </button>
      {menu.node}
    </div>
  );
}

// ---------- 值输入：随字段类型和运算符变化 ----------

const SIZE_UNITS = ["B", "KB", "MB", "GB"];
const SPAN_UNITS: { value: string; label: string }[] = [
  { value: "m", label: t("分钟") },
  { value: "h", label: t("小时") },
  { value: "d", label: t("天") },
  { value: "w", label: t("周") },
  { value: "mo", label: t("个月") },
];

function parseUnit(v: unknown, units: string[], fallback: string): [string, string] {
  const m = String(v ?? "").match(/^\s*([\d.]+)\s*([a-zA-Z]*)\s*$/);
  if (!m) return ["", fallback];
  const u = units.find((x) => x.toLowerCase() === m[2].toLowerCase());
  return [m[1], u ?? (m[2] ? fallback : units[0])];
}

function UnitInput({
  value,
  onChange,
  units,
  fallback,
}: {
  value: unknown;
  onChange: (v: string) => void;
  units: { value: string; label: string }[];
  fallback: string;
}) {
  const [n, u] = parseUnit(
    value,
    units.map((x) => x.value),
    fallback,
  );
  return (
    <span className="row" style={{ gap: 6 }}>
      <input className="input num" inputMode="decimal" value={n} onChange={(e) => onChange(`${e.target.value}${u}`)} />
      <select className="input" value={u} onChange={(e) => onChange(`${n}${e.target.value}`)}>
        {units.map((x) => (
          <option key={x.value} value={x.value}>
            {x.label}
          </option>
        ))}
      </select>
    </span>
  );
}

function ListInput({ value, onChange, mono }: { value: unknown; onChange: (v: string[]) => void; mono?: boolean }) {
  const initial = Array.isArray(value) ? value.join(", ") : String(value ?? "");
  const [text, setText] = useState(initial);
  useEffect(() => {
    // 外部变化（如切换字段）时同步
    if (
      text
        .split(/[,，]/)
        .map((s) => s.trim())
        .filter(Boolean)
        .join(", ") !== initial
    )
      setText(initial);
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [initial]);
  return (
    <input
      className={`input grow ${mono ? "mono" : ""}`}
      placeholder={t("多个值用逗号分隔")}
      value={text}
      onChange={(e) => {
        setText(e.target.value);
        onChange(
          e.target.value
            .split(/[,，]/)
            .map((s) => s.trim())
            .filter(Boolean),
        );
      }}
    />
  );
}

function ValueInput({
  field,
  op,
  value,
  onChange,
}: {
  field: FieldDef | undefined;
  op: string;
  value: unknown;
  onChange: (v: unknown) => void;
}) {
  const ty = field?.type ?? "text";
  const boolSelect = (yes: string, no: string) => (
    <select className="input" value={value === false ? "false" : "true"} onChange={(e) => onChange(e.target.value === "true")}>
      <option value="true">{yes}</option>
      <option value="false">{no}</option>
    </select>
  );
  if (op === "exists") return boolSelect(t("有"), t("没有"));
  if (op === "empty") return boolSelect(t("为空"), t("不为空"));
  if (ty === "bool") return boolSelect(tv("是"), tv("否"));

  if (ty === "enum") {
    if (op === "in" || op === "not_in") {
      const sel = new Set(Array.isArray(value) ? (value as string[]) : []);
      return (
        <span className="chip-select grow">
          {field!.options.map((o) => (
            <button
              key={o}
              className={`chip ${sel.has(o) ? "accent" : ""}`}
              onClick={() => {
                const next = new Set(sel);
                if (next.has(o)) next.delete(o);
                else next.add(o);
                onChange([...next]);
              }}
            >
              {t(o)}
            </button>
          ))}
        </span>
      );
    }
    return (
      <select className="input grow" value={String(value ?? "")} onChange={(e) => onChange(e.target.value)}>
        {field!.options.map((o) => (
          <option key={o} value={o}>
            {t(o)}
          </option>
        ))}
      </select>
    );
  }

  if (ty === "text" || ty === "list") {
    if (op === "in" || op === "not_in") return <ListInput value={value} onChange={onChange} mono={field?.key === "ext"} />;
    const mono = op === "matches" || op === "glob";
    const placeholder =
      op === "matches"
        ? t("正则，如 ^IMG_(\\d{4})")
        : op === "glob"
          ? t("通配符，如 *发票*.pdf")
          : field?.key === "ext"
            ? t("如 pdf")
            : t("可写 {变量}");
    return (
      <input
        className={`input grow ${mono ? "mono" : ""}`}
        placeholder={placeholder}
        value={String(value ?? "")}
        onChange={(e) => onChange(e.target.value)}
      />
    );
  }

  if (ty === "size") {
    const units = SIZE_UNITS.map((u) => ({ value: u, label: u }));
    if (op === "between") {
      const [a, b] = Array.isArray(value) ? value : ["1MB", "10MB"];
      return (
        <span className="row grow" style={{ gap: 6 }}>
          <UnitInput value={a} units={units} fallback="MB" onChange={(v) => onChange([v, b])} />
          <span className="muted">~</span>
          <UnitInput value={b} units={units} fallback="MB" onChange={(v) => onChange([a, v])} />
        </span>
      );
    }
    return <UnitInput value={value} units={units} fallback="MB" onChange={onChange} />;
  }

  if (ty === "number") {
    const num = (v: unknown, set: (x: number | string) => void) => (
      <input
        className="input num"
        inputMode="decimal"
        value={String(v ?? "")}
        onChange={(e) => set(e.target.value === "" || isNaN(Number(e.target.value)) ? e.target.value : Number(e.target.value))}
      />
    );
    const unit = field?.unit && <span className="muted">{field.unit}</span>;
    if (op === "between") {
      const [a, b] = Array.isArray(value) ? value : [0, 100];
      return (
        <span className="row grow" style={{ gap: 6 }}>
          {num(a, (x) => onChange([x, b]))}
          <span className="muted">~</span>
          {num(b, (x) => onChange([a, x]))}
          {unit}
        </span>
      );
    }
    return (
      <span className="row grow" style={{ gap: 6 }}>
        {num(value, onChange)}
        {unit}
      </span>
    );
  }

  // 日期
  if (op === "older_than" || op === "newer_than") {
    return (
      <span className="row grow" style={{ gap: 6 }}>
        <UnitInput value={value} units={SPAN_UNITS} fallback="d" onChange={onChange} />
        <span className="muted">{op === "older_than" ? t("之前") : t("之内")}</span>
      </span>
    );
  }
  if (op === "within") {
    return (
      <select className="input grow" value={String(value)} onChange={(e) => onChange(e.target.value)}>
        {PERIODS.map((p) => (
          <option key={p.value} value={p.value}>
            {p.label}
          </option>
        ))}
      </select>
    );
  }
  if (op === "between") {
    const [a, b] = Array.isArray(value) ? value : ["", ""];
    return (
      <span className="row grow" style={{ gap: 6 }}>
        <input type="date" className="input" value={String(a)} onChange={(e) => onChange([e.target.value, b])} />
        <span className="muted">~</span>
        <input type="date" className="input" value={String(b)} onChange={(e) => onChange([a, e.target.value])} />
      </span>
    );
  }
  return <input type="date" className="input grow" value={String(value ?? "")} onChange={(e) => onChange(e.target.value)} />;
}
