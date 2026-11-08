import { FlaskConical } from "lucide-react";
import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText, type RegexSession } from "../api";
import { t } from "../i18n";

export default function RegexTester({
  pattern = "",
  caseSensitive = false,
  onApply,
}: {
  pattern?: string;
  caseSensitive?: boolean;
  onApply?: (pattern: string, caseSensitive: boolean) => void;
}) {
  const [id] = useState(() => crypto.randomUUID());
  const [error, setError] = useState<string | null>(null);
  const [opening, setOpening] = useState(false);
  const callback = useRef(onApply);
  callback.current = onApply;
  const alive = useRef(false);
  const ready = useRef<Promise<() => void> | null>(null);
  useEffect(() => {
    alive.current = true;
    const listener = listen<RegexSession>(`regex-result-${id}`, (event) => {
      if (alive.current && event.payload.id === id) callback.current?.(event.payload.pattern, event.payload.caseSensitive);
    });
    ready.current = listener;
    void listener.catch(() => {});
    return () => {
      alive.current = false;
      void listener.then((un) => un()).catch(() => {});
      void api.closeRegexLab(id).catch(() => {});
    };
  }, [id]);
  const open = async () => {
    setOpening(true);
    setError(null);
    try {
      await ready.current;
      if (!alive.current) return;
      await api.openRegexLab({ id, pattern, caseSensitive, canApply: !!onApply });
      if (!alive.current) await api.closeRegexLab(id);
    } catch (e) {
      if (alive.current) setError(errorText(e));
    } finally {
      if (alive.current) setOpening(false);
    }
  };
  return (
    <>
      <button type="button" className="icon-btn" disabled={opening} title={t("正则助手")} onClick={() => void open()}>
        <FlaskConical size={16} />
      </button>
      {error && (
        <span className="text-danger" role="alert" title={error}>
          {error}
        </span>
      )}
    </>
  );
}
