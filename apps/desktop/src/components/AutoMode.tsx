// 自动整理模式：手动 / 提醒 / 自动。放在整理页顶部。
// 切到“自动”前先确认：列出会被监听的文件夹和规则，并预览现在开启会立刻整理哪些文件。
import { listen } from "@tauri-apps/api/event";
import { ArrowRight, Bell, Folder, Hand, Trash, TriangleAlert, Zap } from "lucide-react";
import { useEffect, useState } from "react";
import { api, baseName, errorText, type AutoMode as Mode, type PlanView, type WatchStatus } from "../api";
import { tilde } from "../rules";
import { useStore } from "../store";
import { Modal, Segmented, Spinner } from "./ui";

const HINT: Record<Mode, string> = {
  off: "只在你点“扫描”时整理",
  notify: "后台监听文件夹，有可以整理的文件时发通知，由你确认",
  run: "后台监听文件夹并直接整理，可在历史里撤销；移到回收站仍需你确认",
};

export default function AutoMode({ watch }: { watch: WatchStatus | null }) {
  const { config, save } = useStore();
  const mode: Mode = config.settings?.auto ?? "off";
  const [error, setError] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);

  const setMode = async (v: Mode) => {
    setError(null);
    try {
      await save({ ...config, settings: { ...config.settings, auto: v === "off" ? undefined : v } });
    } catch (e) {
      setError(errorText(e));
    }
  };

  // 托盘里选了“自动整理”：也先确认
  useEffect(() => {
    let un: (() => void) | undefined;
    let disposed = false;
    listen("confirm-auto", () => setConfirming(true))
      .then((u) => (disposed ? u() : (un = u)))
      .catch(() => {});
    return () => {
      disposed = true;
      un?.();
    };
  }, []);

  const status = (() => {
    if (error) return error;
    if (mode === "off" || !watch) return HINT[mode];
    if (watch.error) return watch.error;
    const parts = [HINT[mode]];
    if (watch.last_check) parts.push(`${watch.last_check} 检查过`);
    if (watch.last_done) parts.push(`上次自动整理了 ${watch.last_done} 个文件`);
    return parts.join("\n");
  })();

  return (
    <div className="auto-mode" title={status}>
      <Segmented
        value={mode}
        onChange={(v) => (v === "run" ? setConfirming(true) : void setMode(v))}
        options={[
          { value: "off", label: <><Hand size={14} /> 手动</> },
          { value: "notify", label: <><Bell size={14} /> 提醒</> },
          { value: "run", label: <><Zap size={14} /> 自动</> },
        ]}
      />
      {confirming && mode !== "run" && (
        <ConfirmAuto
          onCancel={() => setConfirming(false)}
          onConfirm={async () => {
            await setMode("run");
            setConfirming(false);
          }}
        />
      )}
    </div>
  );
}

function ConfirmAuto({ onCancel, onConfirm }: { onCancel: () => void; onConfirm: () => Promise<void> }) {
  const { config } = useStore();
  const rules = config.rules ?? [];
  const [plan, setPlan] = useState<PlanView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(false);

  useEffect(() => {
    api.scan().then(setPlan).catch((e) => setError(errorText(e)));
  }, []);

  // 每个文件夹实际会用到的规则（启用的）
  const folders = (config.folders ?? []).map((f) => ({
    path: f.path,
    rules: rules.filter((r) => r.enabled !== false && f.rules?.includes(r.name)).map((r) => r.name),
  }));
  const watched = folders.filter((f) => f.rules.length > 0);

  // 预览：自动整理只做可以撤销的操作，含“移到回收站”的文件留给用户确认
  const groups = new Map<number, PlanView["ops"]>();
  for (const o of plan?.ops ?? []) groups.set(o.group, [...(groups.get(o.group) ?? []), o]);
  const all = [...groups.values()];
  const held = all.filter((ops) => ops.some((o) => o.kind === "trash"));
  const now = all.filter((ops) => !ops.some((o) => o.kind === "trash"));
  const SHOW = 6;

  return (
    <Modal
      title="开启自动整理"
      icon={<Zap size={18} color="var(--accent)" />}
      onClose={onCancel}
      width={560}
      footer={
        <>
          <span className="spacer" />
          <button className="btn" onClick={onCancel}>
            取消
          </button>
          <button
            className="btn primary"
            disabled={busy || watched.length === 0}
            onClick={async () => {
              setBusy(true);
              await onConfirm();
            }}
          >
            {busy ? <Spinner size={14} /> : <Zap size={14} />} 开启自动整理
          </button>
        </>
      }
    >
      <p className="text-2" style={{ marginBottom: 12 }}>
        Foldo 会在后台监听下面的文件夹，有新文件时直接按规则整理。整理过的都可以在“历史”里撤销；移到回收站的文件仍会先问你。
      </p>

      <div className="confirm-section">
        <div className="section-title">监听的文件夹</div>
        {watched.length === 0 && (
          <div className="banner warn">
            <TriangleAlert size={16} />
            还没有文件夹添加了启用的规则，开启后不会整理任何文件。
          </div>
        )}
        {folders.map((f) => (
          <div key={f.path} className={`confirm-folder ${f.rules.length ? "" : "off"}`} title={tilde(f.path)}>
            <Folder size={15} />
            <b>{baseName(f.path)}</b>
            <span className="muted ellipsis">{f.rules.length ? f.rules.join("、") : "没有规则，不整理"}</span>
          </div>
        ))}
      </div>

      <div className="confirm-section">
        <div className="section-title">现在开启会立刻整理</div>
        {error && <div className="banner error selectable">{error}</div>}
        {!plan && !error && (
          <div className="row muted">
            <Spinner size={14} /> 正在预览…
          </div>
        )}
        {plan && now.length === 0 && <div className="muted">目前没有需要整理的文件，之后有新文件时才会整理。</div>}
        {now.slice(0, SHOW).map((ops) => {
          const last = ops[ops.length - 1];
          return (
            <div key={last.group} className="confirm-file">
              <span className="ellipsis">{last.origin_name}</span>
              <ArrowRight size={13} className="muted" />
              <span className="muted ellipsis mono">{last.dst_dir_display ?? ""}</span>
            </div>
          );
        })}
        {now.length > SHOW && <div className="muted">还有 {now.length - SHOW} 个文件…</div>}
        {held.length > 0 && (
          <div className="row muted" style={{ marginTop: 6 }}>
            <Trash size={14} /> {held.length} 个要移到回收站的文件不会自动处理，会提醒你确认
          </div>
        )}
      </div>
    </Modal>
  );
}
