// 撤销预览：先告诉用户哪些能恢复、哪些不能（以及原因），确认后再撤销。
import { CircleCheck, RotateCcw, Trash, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { api, errorText, KIND_LABEL, type UndoCheck, type UndoView } from "../api";
import { Modal, Spinner } from "./ui";
import { t } from "../i18n";

export default function UndoDialog({ batchId, onClose, onDone }: { batchId: number; onClose: () => void; onDone: (r: UndoView) => void }) {
  const [checks, setChecks] = useState<UndoCheck[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api
      .undoPreview(batchId)
      .then(setChecks)
      .catch((e) => setError(errorText(e)));
  }, [batchId]);

  const restore = checks?.filter((c) => c.outcome === "restore") ?? [];
  const trash = checks?.filter((c) => c.outcome === "in_trash") ?? [];
  const blocked = checks?.filter((c) => c.outcome === "blocked") ?? [];

  const run = async () => {
    setBusy(true);
    try {
      onDone(await api.undo(batchId));
    } catch (e) {
      setError(errorText(e));
      setBusy(false);
    }
  };

  return (
    <Modal
      title={t("撤销第 {0} 次整理", [batchId])}
      subtitle={t("把这次移动过的文件放回原处")}
      onClose={onClose}
      footer={
        <>
          <span className="spacer" />
          <button className="btn" onClick={onClose}>
            {t("取消")}
          </button>
          <button data-testid="undo-confirm" className="btn primary" disabled={!checks || restore.length === 0 || busy} onClick={run}>
            {busy ? <Spinner size={14} /> : <RotateCcw size={15} />}
            {t("恢复 {0} 项", [restore.length])}
          </button>
        </>
      }
    >
      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          {error}
        </div>
      )}
      {!checks && !error && (
        <div className="row muted">
          <Spinner /> {t("正在检查…")}
        </div>
      )}
      {checks && (
        <>
          <div className="stats">
            <div className="stat">
              <div className="num" style={{ color: "var(--ok)" }}>
                {restore.length}
              </div>
              <div className="label">{t("可以恢复")}</div>
            </div>
            <div className="stat">
              <div className="num">{trash.length}</div>
              <div className="label">{t("在回收站，需手动还原")}</div>
            </div>
            <div className="stat">
              <div className="num" style={{ color: blocked.length ? "var(--warn)" : undefined }}>
                {blocked.length}
              </div>
              <div className="label">{t("无法恢复")}</div>
            </div>
          </div>
          <div className="card">
            {[...blocked, ...trash, ...restore].map((c, i) => (
              <div key={i} className="list-row">
                {c.outcome === "restore" ? (
                  <CircleCheck size={17} color="var(--ok)" />
                ) : c.outcome === "in_trash" ? (
                  <Trash size={17} color="var(--muted)" />
                ) : (
                  <TriangleAlert size={17} color="var(--warn)" />
                )}
                <div className="row-main">
                  <div className="row-title ellipsis">
                    {KIND_LABEL[c.kind] ?? c.kind} · {c.name}
                  </div>
                  <div className="row-sub ellipsis">{c.reason ?? `${c.dst_display ?? ""} → ${c.src_display}`}</div>
                </div>
              </div>
            ))}
            {checks.length === 0 && <div className="list-row muted">{t("这次整理没有可撤销的操作。")}</div>}
          </div>
        </>
      )}
    </Modal>
  );
}
