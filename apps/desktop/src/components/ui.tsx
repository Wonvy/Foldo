import {
  Archive,
  Box,
  Code,
  File,
  FileMusic,
  FileSpreadsheet,
  FileText,
  Film,
  Image,
  LoaderCircle,
  Package,
  PenTool,
  Type,
  X,
  type LucideIcon,
} from "lucide-react";
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";

// ---------- 文件类型图标 ----------

const CATEGORY_STYLE: Record<string, { icon: LucideIcon; color: string }> = {
  图片: { icon: Image, color: "#e8730c" },
  文档: { icon: FileText, color: "#3d63f5" },
  表格与演示: { icon: FileSpreadsheet, color: "#16a34a" },
  压缩包: { icon: Archive, color: "#b7791f" },
  安装包: { icon: Package, color: "#7c3aed" },
  视频: { icon: Film, color: "#db2777" },
  音频: { icon: FileMusic, color: "#0d9488" },
  设计文件: { icon: PenTool, color: "#c026d3" },
  "3D 模型": { icon: Box, color: "#4f46e5" },
  代码: { icon: Code, color: "#64748b" },
  字体: { icon: Type, color: "#a16207" },
};

export function FileIcon({ category, size = 32 }: { category: string | null | undefined; size?: number }) {
  const s = (category && CATEGORY_STYLE[category]) || { icon: File, color: "#8a8a94" };
  const Icon = s.icon;
  return (
    <span
      className="file-icon"
      style={{ width: size, height: size, background: `${s.color}1c`, color: s.color }}
    >
      <Icon size={Math.round(size * 0.5)} strokeWidth={1.9} />
    </span>
  );
}

// ---------- 基础控件 ----------

export function Switch({
  on,
  onChange,
  label,
  disabled,
}: {
  on: boolean;
  onChange: (v: boolean) => void;
  label?: string;
  disabled?: boolean;
}) {
  return (
    <button
      type="button"
      role="switch"
      aria-checked={on}
      aria-label={label}
      disabled={disabled}
      className={`switch ${on ? "on" : ""}`}
      onClick={(e) => {
        e.stopPropagation();
        onChange(!on);
      }}
    />
  );
}

export function Segmented<T extends string>({
  value,
  options,
  onChange,
}: {
  value: T;
  options: { value: T; label: ReactNode }[];
  onChange: (v: T) => void;
}) {
  return (
    <div className="segmented" role="radiogroup">
      {options.map((o) => (
        <button
          key={o.value}
          type="button"
          role="radio"
          aria-checked={value === o.value}
          className={value === o.value ? "on" : ""}
          onClick={() => onChange(o.value)}
        >
          {o.label}
        </button>
      ))}
    </div>
  );
}

/** 支持“部分选中”的复选框。 */
export function Checkbox({
  checked,
  indeterminate,
  onChange,
  label,
}: {
  checked: boolean;
  indeterminate?: boolean;
  onChange: (v: boolean) => void;
  label?: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  useEffect(() => {
    if (ref.current) ref.current.indeterminate = !!indeterminate && !checked;
  }, [indeterminate, checked]);
  return (
    <input
      ref={ref}
      type="checkbox"
      className="checkbox"
      aria-label={label}
      checked={checked}
      onClick={(e) => e.stopPropagation()}
      onChange={(e) => onChange(e.target.checked)}
    />
  );
}

export function Spinner({ size = 16 }: { size?: number }) {
  return <LoaderCircle size={size} className="spin" />;
}

// ---------- 对话框 ----------

export function Modal({
  title,
  subtitle,
  icon,
  onClose,
  children,
  footer,
  width,
}: {
  title: ReactNode;
  subtitle?: ReactNode;
  icon?: ReactNode;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
  width?: number;
}) {
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return createPortal(
    <div className="modal-backdrop" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="modal" role="dialog" aria-modal style={width ? { width: `min(${width}px, 100%)` } : undefined}>
        <div className="modal-head">
          {icon}
          <div style={{ flex: 1, minWidth: 0 }}>
            <h2>{title}</h2>
            {subtitle && <div className="muted ellipsis" style={{ marginTop: 2 }}>{subtitle}</div>}
          </div>
          <button className="icon-btn" onClick={onClose} aria-label="关闭">
            <X size={17} />
          </button>
        </div>
        <div className="modal-body">{children}</div>
        {footer && <div className="modal-foot">{footer}</div>}
      </div>
    </div>,
    document.body,
  );
}

// ---------- 弹出菜单 ----------

export type MenuItem =
  | { label: string; icon?: ReactNode; onClick: () => void; danger?: boolean }
  | "divider";

export function Menu({
  anchor,
  items,
  onClose,
}: {
  anchor: DOMRect;
  items: MenuItem[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState({ top: anchor.bottom + 4, left: anchor.right - 220 });

  // 放不下时向上展开、向左对齐
  useLayoutEffect(() => {
    const el = ref.current;
    if (!el) return;
    const r = el.getBoundingClientRect();
    let top = anchor.bottom + 4;
    if (top + r.height > window.innerHeight - 8) top = anchor.top - r.height - 4;
    const left = Math.max(8, Math.min(anchor.right - r.width, window.innerWidth - r.width - 8));
    setPos({ top, left });
  }, [anchor]);

  useEffect(() => {
    const close = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) onClose();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("mousedown", close);
    window.addEventListener("keydown", onKey);
    window.addEventListener("resize", onClose);
    return () => {
      window.removeEventListener("mousedown", close);
      window.removeEventListener("keydown", onKey);
      window.removeEventListener("resize", onClose);
    };
  }, [onClose]);

  return createPortal(
    <div className="menu" ref={ref} style={pos} role="menu">
      {items.map((it, i) =>
        it === "divider" ? (
          <hr key={i} />
        ) : (
          <button
            key={i}
            role="menuitem"
            className={it.danger ? "danger" : ""}
            onClick={() => {
              onClose();
              it.onClick();
            }}
          >
            {it.icon}
            {it.label}
          </button>
        ),
      )}
    </div>,
    document.body,
  );
}

/** 管理“哪个菜单开着”的小工具。 */
export function useMenu() {
  const [menu, setMenu] = useState<{ anchor: DOMRect; items: MenuItem[] } | null>(null);
  return {
    open: (el: HTMLElement, items: MenuItem[]) => setMenu({ anchor: el.getBoundingClientRect(), items }),
    node: menu && <Menu anchor={menu.anchor} items={menu.items} onClose={() => setMenu(null)} />,
  };
}

// ---------- 轻提示 ----------

export function Toast({
  children,
  actions,
  onClose,
  timeout = 8000,
}: {
  children: ReactNode;
  actions?: ReactNode;
  onClose: () => void;
  timeout?: number;
}) {
  useEffect(() => {
    const t = setTimeout(onClose, timeout);
    return () => clearTimeout(t);
  }, [onClose, timeout]);
  return createPortal(
    <div className="toast" role="status">
      <span>{children}</span>
      {actions}
    </div>,
    document.body,
  );
}
