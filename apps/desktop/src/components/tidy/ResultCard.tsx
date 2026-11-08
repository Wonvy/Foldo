// 执行结果：成功、跳过、失败的数量，可以撤销这次整理。
import { CircleCheck, RotateCcw, TriangleAlert } from "lucide-react";
import type { ExecView, UndoView } from "../../api";
import { t } from "../../i18n";

export default function ResultCard({
  result,
  undone,
  onUndo,
  onContinue,
}: {
  result: ExecView;
  /** 撤销后的结果；还没撤销时为 null */
  undone: UndoView | null;
  onUndo: () => void;
  onContinue: () => void;
}) {
  return (
    <div className="card">
      <div className="result-hero" data-testid="result">
        <div className={`art ${result.failed.length ? "warn" : ""}`}>
          {result.failed.length ? <TriangleAlert size={26} /> : <CircleCheck size={28} />}
        </div>
        <h2>{undone ? t("已撤销，恢复了 {0} 项", [undone.restored]) : t("整理完成：成功 {0} 项", [result.done])}</h2>
        <p className="muted">
          {result.skipped.length > 0 && t("跳过 {0} 项（执行时出现了新冲突）。", [result.skipped.length])}
          {result.failed.length > 0 && t("失败 {0} 项。", [result.failed.length])}
          {!undone && result.skipped.length + result.failed.length === 0 && t("所有选中的文件都已放到位。")}
        </p>
        <div className="row" style={{ marginTop: 10 }}>
          {!undone && result.done > 0 && (
            <button data-testid="undo-result" className="btn" onClick={onUndo}>
              <RotateCcw size={15} /> {t("撤销这次整理")}
            </button>
          )}
          <button className="btn primary" onClick={onContinue}>
            {t("继续整理")}
          </button>
        </div>
      </div>
      {[...result.skipped.map((p) => ({ ...p, failed: false })), ...result.failed.map((p) => ({ ...p, failed: true }))].map((p, i) => (
        <div key={i} className="list-row">
          <span className={`chip ${p.failed ? "danger" : "warn"}`}>{p.failed ? t("失败") : t("跳过")}</span>
          <div className="row-main">
            <div className="row-title ellipsis">{p.name}</div>
            <div className="row-sub">{p.error}</div>
          </div>
        </div>
      ))}
      {undone?.in_trash.map((p) => (
        <div key={p} className="list-row">
          <span className="chip">{t("回收站")}</span>
          <div className="row-main">
            <div className="row-sub">{t("需要到回收站手动还原：{0}", [p])}</div>
          </div>
        </div>
      ))}
      <div style={{ height: 12 }} />
    </div>
  );
}
