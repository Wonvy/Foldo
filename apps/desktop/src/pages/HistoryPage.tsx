import { ChevronDown, ChevronRight, Clock, RefreshCw, RotateCcw, TriangleAlert } from "lucide-react";
import { useCallback, useEffect, useState } from "react";
import { api, errorText, KIND_LABEL, STATUS_LABEL, type Batch, type OpRecord, type Stats, type UndoView } from "../api";
import { formatBytes } from "../rules";
import UndoDialog from "../components/UndoDialog";
import { t } from "../i18n";

const STATUS_CHIP: Record<string, string> = {
  done: "ok",
  undone: "",
  failed: "danger",
  partial: "warn",
  partially_undone: "warn",
  running: "warn",
};

export default function HistoryPage({ active }: { active: boolean }) {
  const [batches, setBatches] = useState<Batch[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [open, setOpen] = useState<number | null>(null);
  const [ops, setOps] = useState<OpRecord[]>([]);
  const [undoFor, setUndoFor] = useState<number | null>(null);
  const [lastUndo, setLastUndo] = useState<UndoView | null>(null);

  const [stats, setStats] = useState<Stats | null>(null);
  const refresh = useCallback(() => {
    api
      .stats()
      .then(setStats)
      .catch(() => setStats(null));
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
    if (open === id) return setOpen(null);
    setOpen(id);
    setOps([]);
    try {
      setOps((await api.batchOps(id)).filter((o) => o.kind !== "mkdir"));
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>{t("历史")}</h1>
          <p className="sub">{t("每次整理都会记录下来，可以查看明细或撤销。")}</p>
        </div>
        <button className="btn" onClick={refresh}>
          <RefreshCw size={15} /> {t("刷新")}
        </button>
      </div>

      {stats && stats.files + stats.auto_runs + stats.manual_runs > 0 && (
        <div className="card stats-card">
          <div className="stat-cell">
            <div className="num">{stats.files}</div>
            <div className="muted">{t("本月整理的文件")}</div>
          </div>
          <div className="stat-cell">
            <div className="num">{formatBytes(stats.trashed_bytes)}</div>
            <div className="muted">{t("移到回收站腾出的空间（{0} 个文件）", [stats.trashed])}</div>
          </div>
          <div className="stat-cell">
            <div className="num">
              {stats.auto_runs} / {stats.manual_runs}
            </div>
            <div className="muted">{t("自动 / 手动整理次数")}</div>
          </div>
          {stats.top_rules.length > 0 && (
            <div className="stat-cell wide">
              <div className="muted">{t("用得最多的规则")}</div>
              <div className="ellipsis">{stats.top_rules.map(([name, n]) => t("{0}（{1} 次）", [name, n])).join(t("、"))}</div>
            </div>
          )}
        </div>
      )}

      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          {error}
        </div>
      )}

      {lastUndo && (
        <div className={`banner ${lastUndo.failed.length ? "warn" : "info"}`}>
          <RotateCcw size={16} />
          <span>
            {t("已撤销第 {0} 次整理，恢复 {1} 项", [lastUndo.batch_id, lastUndo.restored])}
            {lastUndo.in_trash.length > 0 && t("；{0} 项需要到回收站手动还原", [lastUndo.in_trash.length])}
            {lastUndo.failed.length > 0 && t("；{0} 项无法恢复", [lastUndo.failed.length])}
            {t("。")}
          </span>
        </div>
      )}

      {batches?.length === 0 && (
        <div className="empty">
          <div className="art">
            <Clock size={24} />
          </div>
          <h2>{t("还没有整理记录")}</h2>
          <p className="muted">{t("执行整理后，记录会出现在这里。")}</p>
        </div>
      )}

      {batches && batches.length > 0 && (
        <div className="card">
          {batches.map((b) => (
            <div key={b.id}>
              <div className="list-row clickable" onClick={() => expand(b.id)}>
                {open === b.id ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
                <div className="row-main">
                  <div className="row-title">
                    {b.started_at.slice(0, 16)}
                    <span className="muted" style={{ fontWeight: 400 }}>
                      {"  "}#{b.id} · {b.source === "gui" ? t("图形界面") : b.source === "cli" ? t("命令行") : b.source}
                    </span>
                  </div>
                  <div className="row-sub">{t("{0} 项操作", [b.op_count])}</div>
                </div>
                <span className={`chip ${STATUS_CHIP[b.status] ?? ""}`}>{STATUS_LABEL[b.status] ?? b.status}</span>
                {b.status !== "undone" && b.op_count > 0 && (
                  <button
                    className="btn sm"
                    onClick={(e) => {
                      e.stopPropagation();
                      setUndoFor(b.id);
                    }}
                  >
                    <RotateCcw size={14} /> {t("撤销")}
                  </button>
                )}
              </div>
              {open === b.id && (
                <div style={{ background: "var(--surface-2)", borderTop: "1px solid var(--border)" }}>
                  {ops.map((o) => (
                    <div key={o.id} className="list-row" style={{ paddingLeft: 44, minHeight: 40 }}>
                      <span className="chip">{KIND_LABEL[o.kind] ?? o.kind}</span>
                      <div className="row-main">
                        <div className="row-title ellipsis">{o.name}</div>
                        <div className="row-sub ellipsis mono">
                          <button className="path-link" onClick={() => api.reveal(o.src)}>
                            {o.src_display}
                          </button>
                          {o.dst && (
                            <>
                              {"  →  "}
                              <button className="path-link" onClick={() => api.reveal(o.dst!)}>
                                {o.dst_display}
                              </button>
                            </>
                          )}
                        </div>
                        {o.error && (
                          <div className="row-sub" style={{ color: "var(--warn)" }}>
                            {o.error}
                          </div>
                        )}
                      </div>
                      <span className={`chip ${o.status === "done" ? "" : (STATUS_CHIP[o.status] ?? "warn")}`}>
                        {STATUS_LABEL[o.status] ?? o.status}
                      </span>
                    </div>
                  ))}
                </div>
              )}
            </div>
          ))}
        </div>
      )}

      {undoFor !== null && (
        <UndoDialog
          batchId={undoFor}
          onClose={() => setUndoFor(null)}
          onDone={(r) => {
            setUndoFor(null);
            setLastUndo(r);
            refresh();
            if (open === r.batch_id) {
              api.batchOps(r.batch_id).then((x) => setOps(x.filter((o) => o.kind !== "mkdir")));
            }
          }}
        />
      )}
    </div>
  );
}
