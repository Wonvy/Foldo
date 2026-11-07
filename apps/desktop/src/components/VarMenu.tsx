// “插入变量”菜单：按分组列出全部字段和系统变量，可搜索。
import { Braces } from "lucide-react";
import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { createPortal } from "react-dom";
import { useStore } from "../store";

type Item = { key: string; label: string; group: string; desc: string; insert: string };

export default function VarMenu({ onInsert }: { onInsert: (text: string) => void }) {
  const { fields, vars } = useStore();
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const btn = useRef<HTMLButtonElement>(null);
  const pop = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: 0, left: 0 });

  const items = useMemo<Item[]>(
    () => [
      ...fields.map((f) => ({
        key: f.key,
        label: f.label,
        group: f.group,
        desc: f.desc,
        insert: f.type === "date" ? `{${f.key}:%Y-%m-%d}` : `{${f.key}}`,
      })),
      ...vars.map((v) => ({ key: v.key, label: v.label, group: v.group, desc: v.desc, insert: `{${v.key}}` })),
    ],
    [fields, vars],
  );
  const shown = items.filter((i) => !q || `${i.label} ${i.key} ${i.group}`.toLowerCase().includes(q.toLowerCase()));
  const groups = [...new Set(shown.map((i) => i.group))];

  useLayoutEffect(() => {
    if (!open || !btn.current || !pop.current) return;
    const r = btn.current.getBoundingClientRect();
    const h = pop.current.offsetHeight;
    const w = pop.current.offsetWidth;
    const top = r.bottom + h + 8 > window.innerHeight ? Math.max(8, r.top - h - 4) : r.bottom + 4;
    setPos({ top, left: Math.max(8, Math.min(r.right - w, window.innerWidth - w - 8)) });
  }, [open, q]);

  useEffect(() => {
    if (!open) return;
    const close = (e: MouseEvent) => {
      if (!pop.current?.contains(e.target as Node) && !btn.current?.contains(e.target as Node)) setOpen(false);
    };
    window.addEventListener("mousedown", close);
    return () => window.removeEventListener("mousedown", close);
  }, [open]);

  return (
    <>
      <button ref={btn} className="icon-btn" title="插入变量（文件属性、拍摄时间、系统文件夹……）" aria-label="插入变量" onClick={() => setOpen(!open)}>
        <Braces size={16} />
      </button>
      {open &&
        createPortal(
          <div className="menu var-menu" ref={pop} style={pos}>
            <input className="input" autoFocus placeholder="搜索变量，如 拍摄、宽度、桌面" value={q} onChange={(e) => setQ(e.target.value)} />
            <div className="var-list">
              {groups.map((g) => (
                <div key={g}>
                  <div className="section-title" style={{ padding: "8px 8px 2px" }}>
                    {g}
                  </div>
                  {shown
                    .filter((i) => i.group === g)
                    .map((i) => (
                      <button
                        key={i.key + i.insert}
                        title={i.desc}
                        onClick={() => {
                          onInsert(i.insert);
                          setOpen(false);
                        }}
                      >
                        <span>{i.label}</span>
                        <span className="muted mono" style={{ marginLeft: "auto" }}>
                          {i.insert}
                        </span>
                      </button>
                    ))}
                </div>
              ))}
              {shown.length === 0 && <div className="muted" style={{ padding: 10 }}>没有匹配的变量</div>}
            </div>
          </div>,
          document.body,
        )}
    </>
  );
}
