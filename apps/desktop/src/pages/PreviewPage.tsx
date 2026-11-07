import { useEffect, useMemo, useState } from "react";
import { api, errorText, type ExecView, type OpView, type PlanView, type UndoView } from "../api";
import { ConfirmButton, KindBadge, PathLink } from "../components";

type Group = { group: number; rule: string; ops: OpView[] };

export default function PreviewPage({
  text,
  dirty,
  active,
}: {
  text: string;
  dirty: boolean;
  active: boolean;
}) {
  const [ruleNames, setRuleNames] = useState<string[]>([]);
  const [onlyRule, setOnlyRule] = useState<string>("");
  const [plan, setPlan] = useState<PlanView | null>(null);
  const [selected, setSelected] = useState<Set<number>>(new Set());
  const [busy, setBusy] = useState<"plan" | "exec" | "undo" | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [result, setResult] = useState<ExecView | null>(null);
  const [undoResult, setUndoResult] = useState<UndoView | null>(null);

  // 切换到本页时刷新规则名称，供“只运行某条规则”选择
  useEffect(() => {
    if (!active) return;
    api
      .validateRules(text)
      .then((s) => setRuleNames(s.rules.map((r) => r.name)))
      .catch(() => setRuleNames([]));
  }, [active, text]);

  const groups = useMemo<Group[]>(() => {
    if (!plan) return [];
    const map = new Map<number, Group>();
    for (const op of plan.ops) {
      let g = map.get(op.group);
      if (!g) {
        g = { group: op.group, rule: op.rule, ops: [] };
        map.set(op.group, g);
      }
      g.ops.push(op);
    }
    return [...map.values()];
  }, [plan]);

  const byRule = useMemo(() => {
    const m = new Map<string, Group[]>();
    for (const g of groups) m.set(g.rule, [...(m.get(g.rule) ?? []), g]);
    return [...m.entries()];
  }, [groups]);

  const selectedOps = plan ? plan.ops.filter((o) => selected.has(o.group)) : [];
  const trashCount = selectedOps.filter((o) => o.kind === "trash").length;

  const makePlan = async () => {
    setBusy("plan");
    setError(null);
    setResult(null);
    setUndoResult(null);
    try {
      const p = await api.makePlan(text, onlyRule || null);
      setPlan(p);
      setSelected(new Set(Array.from({ length: p.groups }, (_, i) => i)));
    } catch (e) {
      setPlan(null);
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const executeSelected = async () => {
    if (!plan) return;
    setBusy("exec");
    setError(null);
    try {
      setResult(await api.executePlan(plan.plan_id, [...selected]));
      setPlan(null); // 计划已执行，避免重复执行
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const undoLast = async () => {
    if (!result) return;
    setBusy("undo");
    try {
      setUndoResult(await api.undo(result.batch_id));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setBusy(null);
    }
  };

  const toggle = (g: number) =>
    setSelected((prev) => {
      const next = new Set(prev);
      if (next.has(g)) next.delete(g);
      else next.add(g);
      return next;
    });

  const toggleRule = (gs: Group[], on: boolean) =>
    setSelected((prev) => {
      const next = new Set(prev);
      for (const g of gs) {
        if (on) next.add(g.group);
        else next.delete(g.group);
      }
      return next;
    });

  return (
    <div className="preview-page">
      <header className="page-header">
        <div>
          <h1>预览与执行</h1>
          <p className="muted">
            预览不会改动任何文件。
            {dirty && "当前使用编辑器中尚未保存的规则。"}
          </p>
        </div>
        <div className="actions">
          <select value={onlyRule} onChange={(e) => setOnlyRule(e.target.value)}>
            <option value="">全部启用的规则</option>
            {ruleNames.map((n) => (
              <option key={n} value={n}>
                只运行：{n}
              </option>
            ))}
          </select>
          <button className="btn primary" onClick={makePlan} disabled={busy !== null}>
            {busy === "plan" ? "扫描中…" : plan ? "重新预览" : "生成预览"}
          </button>
        </div>
      </header>

      {error && <div className="banner error">{error}</div>}

      {result && (
        <div className={`result-card ${result.failed.length ? "warn" : "ok"}`}>
          <div>
            <strong>
              批次 #{result.batch_id}：完成 {result.done} 项
              {result.failed.length > 0 && `，失败 ${result.failed.length} 项`}
            </strong>
            {result.failed.map((f) => (
              <div key={f.path} className="fail-line">
                ✗ {f.path}（{f.error}）
              </div>
            ))}
            {undoResult && (
              <div className="undo-line">
                已撤销，恢复 {undoResult.restored} 个文件
                {undoResult.in_trash.length > 0 &&
                  `；${undoResult.in_trash.length} 个文件在回收站中，需要手动还原`}
                {undoResult.failed.map((f) => (
                  <div key={f.path} className="fail-line">
                    ✗ {f.path}（{f.error}）
                  </div>
                ))}
              </div>
            )}
          </div>
          {!undoResult && result.done > 0 && (
            <ConfirmButton
              label="撤销本次"
              confirmText="把这次移动过的文件放回原处？"
              onConfirm={undoLast}
              busy={busy === "undo"}
            />
          )}
        </div>
      )}

      {plan && (
        <>
          <div className="stats">
            <span>扫描 {plan.scanned} 个文件</span>
            <span>命中 {plan.matched}</span>
            <span>已在目标位置 {plan.in_place}</span>
            <span>跳过 {plan.skipped.length}</span>
            <span>计划处理 {plan.groups} 个文件</span>
          </div>

          {plan.warnings.map((w) => (
            <div key={w} className="banner warn">
              {w}
            </div>
          ))}

          {plan.groups === 0 ? (
            <div className="empty">没有需要处理的文件。</div>
          ) : (
            <div className="plan-list">
              {byRule.map(([rule, gs]) => {
                const allOn = gs.every((g) => selected.has(g.group));
                return (
                  <section key={rule} className="plan-rule">
                    <label className="plan-rule-head">
                      <input
                        type="checkbox"
                        checked={allOn}
                        onChange={(e) => toggleRule(gs, e.target.checked)}
                      />
                      <strong>{rule}</strong>
                      <span className="muted">{gs.length} 个文件</span>
                    </label>
                    {gs.map((g) => (
                      <label key={g.group} className={`plan-group ${selected.has(g.group) ? "" : "off"}`}>
                        <input
                          type="checkbox"
                          checked={selected.has(g.group)}
                          onChange={() => toggle(g.group)}
                        />
                        <div className="plan-ops">
                          {g.ops.map((op, i) => (
                            <div key={i} className="plan-op">
                              <KindBadge kind={op.kind} />
                              <PathLink path={op.src} label={op.src_display} />
                              <span className="arrow">→</span>
                              {op.dst ? (
                                <span className="dst">{op.dst_display}</span>
                              ) : (
                                <span className="dst">系统回收站</span>
                              )}
                              {op.note && <span className="note">（{op.note}）</span>}
                            </div>
                          ))}
                        </div>
                      </label>
                    ))}
                  </section>
                );
              })}
            </div>
          )}

          {plan.skipped.length > 0 && (
            <details className="skipped">
              <summary>跳过的文件（{plan.skipped.length}）</summary>
              {plan.skipped.map((s) => (
                <div key={s.display + s.rule} className="skip-line">
                  <span>{s.display}</span>
                  <span className="muted">
                    {s.rule}：{s.reason}
                  </span>
                </div>
              ))}
            </details>
          )}

          {plan.groups > 0 && (
            <footer className="exec-bar">
              <span>
                已选 {selected.size} / {plan.groups} 个文件，共 {selectedOps.length} 项操作
              </span>
              <button
                className="btn"
                onClick={() =>
                  setSelected(
                    selected.size === plan.groups
                      ? new Set()
                      : new Set(Array.from({ length: plan.groups }, (_, i) => i)),
                  )
                }
              >
                {selected.size === plan.groups ? "全不选" : "全选"}
              </button>
              <ConfirmButton
                label="执行所选"
                confirmText={
                  <>
                    执行 {selectedOps.length} 项操作？
                    {trashCount > 0 && `其中 ${trashCount} 项会移入回收站，撤销时需手动还原。`}
                  </>
                }
                danger={trashCount > 0}
                disabled={selected.size === 0}
                busy={busy === "exec"}
                onConfirm={executeSelected}
              />
            </footer>
          )}
        </>
      )}

      {!plan && !result && !error && (
        <div className="empty muted">点击“生成预览”，查看规则会如何整理你的文件。</div>
      )}
    </div>
  );
}
