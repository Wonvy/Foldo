import { GripVertical, X } from "lucide-react";
import { useEffect, useRef, useState, type PointerEvent } from "react";
import type { RuleDef } from "../api";

type Target = { name: string; after: boolean };
type Drag = { name: string; pointer: number; x: number; y: number; moved: boolean; order: string[] };

/** 指针拖拽可与 Tauri 的系统文件拖入共存；规则只能在当前文件夹内排序。 */
export default function FolderRuleList({ rules, folder, disabled, onReorder, onEdit, onRemove }: {
  rules: RuleDef[];
  folder: string;
  disabled: boolean;
  onReorder: (names: string[]) => void;
  onEdit: (name: string) => void;
  onRemove: (name: string) => void;
}) {
  const root = useRef<HTMLSpanElement>(null);
  const active = useRef<Drag | null>(null);
  const [dragging, setDragging] = useState<string | null>(null);
  const [target, setTarget] = useState<Target | null>(null);
  const clear = () => { active.current = null; setDragging(null); setTarget(null); };
  useEffect(() => {
    const escape = (e: KeyboardEvent) => { if (e.key === "Escape") clear(); };
    window.addEventListener("keydown", escape);
    window.addEventListener("blur", clear);
    return () => {
      window.removeEventListener("keydown", escape);
      window.removeEventListener("blur", clear);
    };
  }, []);

  // 选择最近的标签和它的左右半边；标签换行时也能定位。
  const locate = (x: number, y: number): Target | null => {
    const bounds = root.current?.getBoundingClientRect();
    if (!bounds || x < bounds.left - 6 || x > bounds.right + 6 || y < bounds.top - 6 || y > bounds.bottom + 6) return null;
    let closest: Target | null = null;
    let distance = Infinity;
    for (const chip of root.current!.querySelectorAll<HTMLElement>("[data-rule-name]")) {
      const r = chip.getBoundingClientRect();
      const dx = Math.max(r.left - x, 0, x - r.right);
      const dy = Math.max(r.top - y, 0, y - r.bottom);
      const d = dx * dx + dy * dy;
      if (d < distance) {
        distance = d;
        closest = { name: chip.dataset.ruleName!, after: x > r.left + r.width / 2 };
      }
    }
    return closest?.name === active.current?.name ? null : closest;
  };
  const move = (e: PointerEvent<HTMLButtonElement>) => {
    const drag = active.current;
    if (!drag || drag.pointer !== e.pointerId) return;
    if (!drag.moved && Math.hypot(e.clientX - drag.x, e.clientY - drag.y) < 5) return;
    drag.moved = true;
    setDragging(drag.name);
    setTarget(locate(e.clientX, e.clientY));
  };
  const finish = (e: PointerEvent<HTMLButtonElement>) => {
    const drag = active.current;
    if (!drag || drag.pointer !== e.pointerId) return;
    const drop = locate(e.clientX, e.clientY);
    clear();
    const names = rules.map((r) => r.name);
    if (disabled || !drag.moved || !drop || JSON.stringify(names) !== JSON.stringify(drag.order)) return;
    const next = names.filter((name) => name !== drag.name);
    next.splice(next.indexOf(drop.name) + (drop.after ? 1 : 0), 0, drag.name);
    if (next.some((name, i) => name !== names[i])) onReorder(next);
  };

  return (
    <span ref={root} className={`folder-rule-list${dragging ? " sorting" : ""}`}>
      {rules.map((rule, index) => (
        <span key={rule.name} data-rule-name={rule.name}
          className={["rule-chip", rule.enabled === false ? "off" : "", dragging === rule.name ? "dragging" : "",
            target?.name === rule.name ? (target.after ? "insert-after" : "insert-before") : ""].join(" ")}>
          <button type="button" className="rule-chip-grip" disabled={disabled || rules.length < 2}
            aria-label={`调整 ${rule.name} 的顺序，当前位置 ${index + 1}，可用左右方向键`}
            title="左右拖动调整顺序，左侧优先；也可使用左右方向键"
            onPointerDown={(e) => {
              if (e.button !== 0 || !e.isPrimary || disabled) return;
              e.preventDefault();
              e.currentTarget.focus();
              active.current = { name: rule.name, pointer: e.pointerId, x: e.clientX, y: e.clientY, moved: false, order: rules.map((r) => r.name) };
              e.currentTarget.setPointerCapture(e.pointerId);
            }}
            onPointerMove={move} onPointerUp={finish} onPointerCancel={clear} onLostPointerCapture={clear}
            onKeyDown={(e) => {
              if (e.key !== "ArrowLeft" && e.key !== "ArrowRight") return;
              e.preventDefault();
              clear();
              const to = index + (e.key === "ArrowLeft" ? -1 : 1);
              if (to < 0 || to >= rules.length) return;
              const names = rules.map((r) => r.name);
              [names[index], names[to]] = [names[to], names[index]];
              onReorder(names);
            }}><GripVertical size={12} /></button>
          <button className="rule-chip-name" disabled={disabled} title={rule.enabled === false ? "已停用，点击编辑" : "编辑规则"} onClick={() => onEdit(rule.name)}>{rule.name}</button>
          <button className="rule-chip-x" disabled={disabled} aria-label={`从 ${folder} 移除 ${rule.name}`} onClick={() => onRemove(rule.name)}><X size={12} /></button>
        </span>
      ))}
    </span>
  );
}
