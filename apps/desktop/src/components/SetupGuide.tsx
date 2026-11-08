// 上手引导：选好文件夹以后，带用户建第一条规则（直接加到文件夹），再看预览、执行。
// 规则都用上之后显示一次“看预览、勾选执行”的提示，执行过或点“知道了”后不再出现。
import { Check, Eye, FolderCheck, Plus, WandSparkles, X } from "lucide-react";
import { useState } from "react";
import { t } from "../i18n";
import { useStore } from "../store";
import RuleEditor from "./RuleEditor";

const DONE_KEY = "foldo.setupDone";

export function setupDone(): boolean {
  try {
    return localStorage.getItem(DONE_KEY) === "1";
  } catch {
    return false;
  }
}

export function markSetupDone() {
  try {
    localStorage.setItem(DONE_KEY, "1");
  } catch {
    /* 存储不可用 */
  }
}

/** 三步上手的进度：选择文件夹 → 建第一条规则 → 预览并整理。 */
export function SetupSteps({ step, children }: { step: number; children?: React.ReactNode }) {
  const steps = [t("选择文件夹"), t("建第一条规则"), t("预览并整理")];
  return (
    <div className="setup-steps">
      {steps.map((s, i) => {
        const n = i + 1;
        const state = n < step ? "done" : n === step ? "active" : "";
        return (
          <span key={s} className={`setup-step ${state}`}>
            <span className="dot">{n < step ? <Check size={12} /> : n}</span>
            {s}
          </span>
        );
      })}
      {children}
    </div>
  );
}

export default function SetupGuide({ onChanged, onDismiss }: { onChanged: () => void; onDismiss: () => void }) {
  const { config, save } = useStore();
  const folders = config.folders ?? [];
  const rules = config.rules ?? [];
  const enabled = rules.filter((r) => r.enabled !== false);
  const ready = folders.some((f) => enabled.some((r) => f.rules?.includes(r.name)));
  const [editing, setEditing] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const step = ready ? 3 : 2;

  // 已有规则但还没加到文件夹：一键全部加到所有文件夹
  const addAll = async () => {
    setError(null);
    try {
      await save({
        ...config,
        folders: folders.map((f) => ({
          ...f,
          rules: [...(f.rules ?? []), ...enabled.map((r) => r.name).filter((n) => !f.rules?.includes(n))],
        })),
      });
      onChanged();
    } catch (e) {
      setError(String(e));
    }
  };

  return (
    <div className="card setup-guide">
      <SetupSteps step={step}>
        {ready && (
          <button className="icon-btn" aria-label={t("知道了")} title={t("知道了")} onClick={onDismiss} style={{ marginLeft: "auto" }}>
            <X size={15} />
          </button>
        )}
      </SetupSteps>

      {step === 2 && rules.length === 0 && (
        <div className="setup-body">
          <WandSparkles size={20} color="var(--accent)" />
          <div style={{ flex: 1 }}>
            <div className="setup-title">{t("建第一条规则")}</div>
            <div className="muted">
              {t(
                "规则写明“什么样的文件 → 怎么处理”，例如“扩展名是 pdf → 移到 文档/”。新建的规则会直接用在上面的文件夹里，执行前都会先给你看预览。",
              )}
            </div>
          </div>
          <button className="btn primary" onClick={() => setEditing(true)}>
            <Plus size={15} /> {t("新建规则")}
          </button>
        </div>
      )}

      {step === 2 && rules.length > 0 && (
        <div className="setup-body">
          <FolderCheck size={20} color="var(--accent)" />
          <div style={{ flex: 1 }}>
            <div className="setup-title">{t("把规则添加到文件夹")}</div>
            <div className="muted">
              {enabled.length
                ? t("已有 {0} 条规则，还没有用在任何文件夹里。可以一次全部加上，也可以在下面的文件夹里点“+ 规则”逐个添加。", [
                    enabled.length,
                  ])
                : t("规则都停用了。到“规则”页启用后，再添加到文件夹。")}
            </div>
          </div>
          {enabled.length > 0 && (
            <button className="btn primary" onClick={addAll}>
              <Plus size={15} /> {t("全部添加")}
            </button>
          )}
        </div>
      )}

      {step === 3 && (
        <div className="setup-body">
          <Eye size={20} color="var(--accent)" />
          <div style={{ flex: 1 }}>
            <div className="setup-title">{t("看看预览")}</div>
            <div className="muted">
              {t("下面是按规则给出的整理建议，什么都还没动。确认后勾选，点底部的按钮执行；执行过的可以在“历史”里撤销。")}
            </div>
          </div>
        </div>
      )}

      {error && <div className="banner error selectable">{error}</div>}

      {editing && (
        <RuleEditor
          index={null}
          assignTo={folders.map((f) => f.path)}
          onClose={() => {
            setEditing(false);
            onChanged();
          }}
          onOpenAdvanced={() => {}}
        />
      )}
    </div>
  );
}
