// 锚定在某个按钮下方的弹出层；放不下时向上展开，点外面或按 Esc 关闭。
import { useEffect, useLayoutEffect, useRef, useState, type ReactNode, type RefObject } from "react";
import { createPortal } from "react-dom";

export default function Popover({
  anchor,
  onClose,
  width = 300,
  children,
}: {
  anchor: RefObject<HTMLElement | null>;
  onClose: () => void;
  width?: number;
  children: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const [pos, setPos] = useState<{ top: number; left: number } | null>(null);

  useLayoutEffect(() => {
    const a = anchor.current?.getBoundingClientRect();
    const h = ref.current?.offsetHeight ?? 0;
    if (!a) return;
    const top = a.bottom + h + 8 > window.innerHeight ? Math.max(8, a.top - h - 4) : a.bottom + 4;
    setPos({ top, left: Math.max(8, Math.min(a.left, window.innerWidth - width - 8)) });
  }, [anchor, width]);

  useEffect(() => {
    const down = (e: MouseEvent) => {
      if (!ref.current?.contains(e.target as Node) && !anchor.current?.contains(e.target as Node)) onClose();
    };
    const key = (e: KeyboardEvent) => {
      if (e.key === "Escape") {
        e.stopPropagation();
        onClose();
      }
    };
    window.addEventListener("mousedown", down);
    window.addEventListener("keydown", key, true);
    return () => {
      window.removeEventListener("mousedown", down);
      window.removeEventListener("keydown", key, true);
    };
  }, [anchor, onClose]);

  return createPortal(
    <div className="menu popover" ref={ref} style={{ width, ...(pos ?? { top: -9999, left: -9999 }) }}>
      {children}
    </div>,
    document.body,
  );
}
