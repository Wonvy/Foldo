// 规则编辑器底栏的两个小标签：整理文件夹里现在有多少文件符合；用一个文件测试。
import { CircleCheck, CircleX, Layers, ScanSearch, TriangleAlert, X } from "lucide-react";
import { useRef, useState } from "react";
import { baseName, type TestView } from "../../api";
import { t } from "../../i18n";
import { tilde } from "../../rules";
import Popover from "../Popover";
import { Spinner } from "../ui";

export function MatchChip({
  preview,
  invalid,
  error,
}: {
  preview: { count: number; names: string[] } | null;
  invalid: boolean;
  error: string | null;
}) {
  const ref = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  if (invalid) return <span className="foot-chip muted">{t("命中 —")}</span>;
  if (error)
    return (
      <span className="foot-chip" title={error}>
        <TriangleAlert size={13} /> {t("预览失败")}
      </span>
    );
  if (!preview)
    return (
      <span className="foot-chip muted">
        <Spinner size={13} />
      </span>
    );
  return (
    <>
      <button
        ref={ref}
        className={`foot-chip ${preview.count ? "accent" : ""}`}
        title={t("整理文件夹里现在符合的文件")}
        onClick={() => preview.count && setOpen(!open)}
      >
        <Layers size={14} /> {t("命中")} {preview.count}
      </button>
      {open && (
        <Popover anchor={ref} onClose={() => setOpen(false)} width={280}>
          <div className="pick-list">
            {preview.names.map((n) => (
              <div key={n} className="pick-item ellipsis">
                {n}
              </div>
            ))}
            {preview.count > preview.names.length && (
              <div className="pick-item muted">{t("还有 {0} 个…", [preview.count - preview.names.length])}</div>
            )}
          </div>
        </Popover>
      )}
    </>
  );
}

export function TestChip({
  path,
  view,
  error,
  onPick,
  onInspect,
  onClear,
}: {
  path: string | null;
  view: TestView | null;
  error: string | null;
  onPick: () => void;
  onInspect: () => void;
  onClear: () => void;
}) {
  if (!path) {
    return (
      <button className="foot-chip" title={t("选一个文件，看每个条件是否成立、会放到哪里")} onClick={onPick}>
        <ScanSearch size={14} /> {t("测试（可拖入文件）")}
      </button>
    );
  }
  const ok = !!view?.destination;
  return (
    <span className={`foot-chip test ${view ? (ok ? "ok" : "no") : ""}`}>
      {error ? <TriangleAlert size={14} /> : !view ? <Spinner size={13} /> : ok ? <CircleCheck size={14} /> : <CircleX size={14} />}
      <button className="link-plain ellipsis" title={t("{0}\n点击查看属性", [path])} onClick={onInspect}>
        {baseName(path)}
      </button>
      {error && (
        <span className="ellipsis" role="alert" title={error} style={{ maxWidth: 220 }}>
          {error}
        </span>
      )}
      {!error && view && (
        <span className="ellipsis" title={view.destination ?? view.reason ?? ""} style={{ maxWidth: 220 }}>
          {ok ? `→ ${tilde(view.destination!)}` : (view.reason ?? t("不满足"))}
        </span>
      )}
      <button className="icon-btn tiny" title={t("换一个文件")} onClick={onPick}>
        <ScanSearch size={13} />
      </button>
      <button className="icon-btn tiny" title={t("结束测试")} onClick={onClear}>
        <X size={13} />
      </button>
    </span>
  );
}
