import { useCallback, useEffect, useState } from "react";
import { api, errorText, STATUS_LABEL, type Batch, type OpRecord, type UndoView } from "../api";
import { ConfirmButton, Icon, KindBadge, PathLink } from "../components";

export default function HistoryPage({ active }: { active: boolean }) {
  const [batches, setBatches] = useState<Batch[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [ops, setOps] = useState<OpRecord[]>([]);
  const [undoing, setUndoing] = useState<number | null>(null);
  const [undoResult, setUndoResult] = useState<UndoView | null>(null);

  const refresh = useCallback(() => {
    api
      .history()
      .then((b) => {
        setBatches(b);
        setError(null);
      })
      .catch((e) => setError(errorText(e)));
  }, []);

  useEffect(() => {
    if (active) refresh();
  }, [active, refresh]);

  const expand = async (id: number) => {
    if (open === id) {
      setOpen(null);
      return;
    }
    setOpen(id);
    setOps([]);
    try {
      setOps(await api.batchOps(id));
    } catch (e) {
      setError(errorText(e));
    }
  };

  const undo = async (id: number) => {
    setUndoing(id);
    setUndoResult(null);
    try {
      setUndoResult(await api.undo(id));
      refresh();
      if (open === id) setOps(await api.batchOps(id));
    } catch (e) {
      setError(errorText(e));
    } finally {
      setUndoing(null);
    }
  };

  return (
    <div className="history-page">
      <header className="page-header">
        <h1>历史</h1>
        <div className="actions">
          <button className="btn" onClick={refresh}>
            <Icon name="refresh" /> 刷新
          </button>
        </div>
      </header>

      {error && <div className="banner error">{error}</div>}

      {undoResult && (
        <div className={`result-card ${undoResult.failed.length ? "warn" : "ok"}`}>
          <div>
            <strong>
              批次 #{undoResult.batch_id} 已撤销：恢复 {undoResult.restored} 个文件
            </strong>
            {undoResult.in_trash.map((p) => (
              <div key={p} className="fail-line">
                在回收站中，需手动还原：{p}
              </div>
            ))}
            {undoResult.failed.map((f) => (
              <div key={f.path} className="fail-line">
                ✗ {f.path}（{f.error}）
              </div>
            ))}
          </div>
        </div>
      )}

      {batches?.length === 0 && <div className="empty muted">还没有执行记录。</div>}

      <div className="batch-list">
        {batches?.map((b) => (
          <div key={b.id} className={`batch ${open === b.id ? "open" : ""}`}>
            <div className="batch-row" onClick={() => expand(b.id)}>
              <span className="batch-id">#{b.id}</span>
              <span>{b.started_at}</span>
              <span className="muted">{b.source === "gui" ? "图形界面" : b.source === "cli" ? "命令行" : b.source}</span>
              <span>{b.op_count} 项</span>
              <span className={`status status-${b.status}`}>{STATUS_LABEL[b.status] ?? b.status}</span>
              <span className="spacer" />
              {b.status !== "undone" && b.op_count > 0 && (
                <span onClick={(e) => e.stopPropagation()}>
                  <ConfirmButton
                    label="撤销"
                    confirmText={`撤销批次 #${b.id}？`}
                    onConfirm={() => undo(b.id)}
                    busy={undoing === b.id}
                  />
                </span>
              )}
            </div>
            {open === b.id && (
              <div className="batch-ops">
                {ops.map((o) => (
                  <div key={o.id} className={`plan-op op-${o.status}`}>
                    <KindBadge kind={o.kind} />
                    <PathLink path={o.src} label={o.src_display} />
                    {o.dst && (
                      <>
                        <span className="arrow">→</span>
                        <PathLink path={o.dst} label={o.dst_display ?? o.dst} />
                      </>
                    )}
                    <span className="muted">{STATUS_LABEL[o.status] ?? o.status}</span>
                    {o.error && <span className="note">（{o.error}）</span>}
                  </div>
                ))}
              </div>
            )}
          </div>
        ))}
      </div>
    </div>
  );
}
