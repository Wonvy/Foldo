import { useState, type ReactNode } from "react";
import { api, KIND_LABEL } from "./api";

const ICONS = {
  folder: "M3 7a2 2 0 0 1 2-2h4l2 2h8a2 2 0 0 1 2 2v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z",
  rules: "M8 6h12M8 12h12M8 18h12M4 6h.01M4 12h.01M4 18h.01",
  play: "M7 5v14l11-7z",
  history: "M3 12a9 9 0 1 0 3-6.7M3 4v5h5M12 7v5l3 2",
  reveal: "M14 4h6v6M20 4l-8 8M18 14v4a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h4",
  refresh: "M20 12a8 8 0 1 1-2.3-5.7M20 4v5h-5",
} as const;

export function Icon({ name }: { name: keyof typeof ICONS }) {
  return (
    <svg
      className="icon"
      viewBox="0 0 24 24"
      fill="none"
      stroke="currentColor"
      strokeWidth="2"
      strokeLinecap="round"
      strokeLinejoin="round"
      aria-hidden
    >
      <path d={ICONS[name]} />
    </svg>
  );
}

export function KindBadge({ kind }: { kind: string }) {
  return <span className={`badge kind-${kind}`}>{KIND_LABEL[kind] ?? kind}</span>;
}

/** 可点击的路径：点击后在 Finder / 资源管理器中显示。 */
export function PathLink({ path, label }: { path: string; label: string }) {
  return (
    <button className="path" title={`在文件管理器中显示：${path}`} onClick={() => api.reveal(path)}>
      {label}
    </button>
  );
}

/** 两步确认：先点按钮，再在出现的确认条里确认。Tauri 的 WebView 里不能依赖 window.confirm。 */
export function ConfirmButton({
  label,
  confirmText,
  onConfirm,
  disabled,
  danger,
  busy,
}: {
  label: string;
  confirmText: ReactNode;
  onConfirm: () => void;
  disabled?: boolean;
  danger?: boolean;
  busy?: boolean;
}) {
  const [asking, setAsking] = useState(false);
  if (!asking) {
    return (
      <button
        className={danger ? "btn danger" : "btn primary"}
        disabled={disabled || busy}
        onClick={() => setAsking(true)}
      >
        {busy ? "处理中…" : label}
      </button>
    );
  }
  return (
    <span className="confirm">
      <span className="confirm-text">{confirmText}</span>
      <button
        className={danger ? "btn danger" : "btn primary"}
        onClick={() => {
          setAsking(false);
          onConfirm();
        }}
      >
        确定
      </button>
      <button className="btn" onClick={() => setAsking(false)}>
        取消
      </button>
    </span>
  );
}
