import { File, Folder, KeyRound, Link, Lock, ShieldCheck, Trash, TriangleAlert } from "lucide-react";
import { useState } from "react";
import { api, baseName, errorText, type RuleFile } from "../api";
import { Segmented, Switch } from "../components/ui";
import { pickFile, pickFolder, useStore } from "../store";
import { t } from "../i18n";

const BUFFERS = [
  { value: "none", label: t("不设") },
  { value: "10m", label: t("10 分钟") },
  { value: "1h", label: t("1 小时") },
  { value: "1d", label: t("1 天") },
];

const BUILTIN_EXCLUDES = [
  { icon: Folder, title: t("已有文件夹"), text: t("作为整体保留，不拆分内部文件；文件夹开启“包含子文件夹”时才整理里面的文件") },
  { icon: KeyRound, title: t("凭证文件"), text: t("私钥、证书、密码库、.env 等默认不移动") },
  { icon: Link, title: t("快捷方式"), text: t(".lnk、.url、.webloc 等保留在原位，也不会影响它们指向的文件") },
  {
    icon: ShieldCheck,
    title: t("系统与隐藏文件"),
    text: t(".DS_Store、Thumbs.db、desktop.ini；隐藏文件在文件夹开启“包含隐藏文件”时才整理"),
  },
  { icon: TriangleAlert, title: t("未完成或正在使用"), text: t("下载中的临时文件、Office 打开时的锁文件、被其他程序占用的文件") },
];

/** 设置页里的“整理选项”：固定保留、新文件缓冲期、空文件夹、始终排除。 */
export default function TidyOptions() {
  const { config, save, setView } = useStore();
  const protect = config.protect ?? [];
  const buffer = config.settings?.buffer ?? "none";
  const [pattern, setPattern] = useState("");
  const [error, setError] = useState<string | null>(null);

  const update = async (next: RuleFile) => {
    setError(null);
    try {
      await save(next);
    } catch (e) {
      setError(errorText(e));
    }
  };

  const addProtect = async (entry: string | null) => {
    if (!entry) return;
    setError(null);
    try {
      setView(await api.addProtect(entry));
    } catch (e) {
      setError(errorText(e));
    }
  };

  return (
    <>
      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          {error}
        </div>
      )}

      <div className="card">
        <div className="card-head">
          <Lock size={16} color="var(--muted)" />
          <h2>{t("固定保留")}</h2>
          <span className="muted">{t("这些文件或文件夹永远不会被移动")}</span>
          <span className="spacer" />
          <button className="btn sm" onClick={async () => addProtect(await pickFile(t("选择要固定保留的文件")))}>
            <File size={14} /> {t("文件…")}
          </button>
          <button className="btn sm" onClick={async () => addProtect(await pickFolder(t("选择要固定保留的文件夹")))}>
            <Folder size={14} /> {t("文件夹…")}
          </button>
        </div>
        {protect.map((p, i) => {
          const isPattern = !(p.startsWith("~") || p.includes("/") || p.includes("\\"));
          return (
            <div key={p} className="list-row">
              {isPattern ? <span className="chip">{t("通配符")}</span> : <Lock size={16} color="var(--muted)" />}
              <div className="row-main">
                <div className="row-title mono ellipsis">{isPattern ? p : baseName(p)}</div>
                {!isPattern && <div className="row-sub mono ellipsis">{p}</div>}
              </div>
              <button
                className="icon-btn"
                aria-label={t("移除")}
                onClick={() => update({ ...config, protect: protect.filter((_, j) => j !== i) })}
              >
                <Trash size={15} />
              </button>
            </div>
          );
        })}
        <div className="list-row">
          <input
            className="input mono"
            style={{ flex: 1 }}
            placeholder={t("按文件名保留，如 *.lic 或 *合同*")}
            value={pattern}
            onChange={(e) => setPattern(e.target.value)}
            onKeyDown={(e) => {
              if (e.key === "Enter" && pattern.trim()) {
                void addProtect(pattern.trim());
                setPattern("");
              }
            }}
          />
          <button
            className="btn sm"
            disabled={!pattern.trim()}
            onClick={() => {
              void addProtect(pattern.trim());
              setPattern("");
            }}
          >
            {t("添加")}
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <h2>{t("新文件缓冲期")}</h2>
          <span className="muted">{t("刚下载或刚修改的文件先不整理，避免打断正在进行的工作")}</span>
        </div>
        <div className="card-body">
          <Segmented
            value={buffer}
            onChange={(v) =>
              update({
                ...config,
                settings: { ...config.settings, buffer: v === "none" ? undefined : v },
              })
            }
            options={BUFFERS}
          />
        </div>
        <div className="card-body row" style={{ borderTop: "1px solid var(--border)" }}>
          <div style={{ flex: 1 }}>
            <div>{t("整理后删除变空的文件夹")}</div>
            <div className="muted" style={{ fontSize: 12.5 }}>
              {t("只删除因为文件被移走而变空的文件夹；撤销时会重建")}
            </div>
          </div>
          <Switch
            label={t("整理后删除变空的文件夹")}
            on={!config.settings?.keep_empty_folders}
            onChange={(on) =>
              update({
                ...config,
                settings: { ...config.settings, keep_empty_folders: on ? undefined : true },
              })
            }
          />
        </div>
        <div className="card-body row" style={{ borderTop: "1px solid var(--border)" }}>
          <div style={{ flex: 1 }}>
            <div>{t("定时整理")}</div>
            <div className="muted" style={{ fontSize: 12.5 }}>
              {t("到时间自动整理一次，只做可以撤销的操作；错过的时间点会在下次打开时补上")}
            </div>
          </div>
          <ScheduleInput
            value={config.settings?.schedule}
            onChange={(schedule) => update({ ...config, settings: { ...config.settings, schedule } })}
          />
        </div>
        <div className="card-body row" style={{ borderTop: "1px solid var(--border)" }}>
          <div style={{ flex: 1 }}>
            <div>{t("可撤销次数")}</div>
            <div className="muted" style={{ fontSize: 12.5 }}>
              {t("默认只允许撤销最近一次整理，最多 3 次，按从新到旧的顺序撤销；不会删除历史记录。")}
            </div>
          </div>
          <select
            className="input"
            style={{ width: "auto" }}
            aria-label={t("可撤销次数")}
            value={config.settings?.undo_depth ?? 1}
            onChange={(e) => update({ ...config, settings: { ...config.settings, undo_depth: Number(e.target.value) } })}
          >
            {[1, 2, 3].map((depth) => (
              <option key={depth} value={depth}>
                {t("{0} 次", [depth])}
              </option>
            ))}
          </select>
        </div>
        <div className="card-body row" style={{ borderTop: "1px solid var(--border)" }}>
          <div style={{ flex: 1 }}>
            <div>{t("历史记录保留")}</div>
            <div className="muted" style={{ fontSize: 12.5 }}>
              {t("更早的整理记录会自动清理，清理后不能再撤销")}
            </div>
          </div>
          <select
            className="input"
            style={{ width: "auto" }}
            value={String(config.settings?.keep_history_days ?? 365)}
            onChange={(e) => {
              const days = Number(e.target.value);
              update({ ...config, settings: { ...config.settings, keep_history_days: days === 365 ? undefined : days } });
            }}
          >
            <option value="30">{t("30 天")}</option>
            <option value="90">{t("90 天")}</option>
            <option value="365">{t("1 年")}</option>
            <option value="0">{t("一直保留")}</option>
          </select>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <ShieldCheck size={16} color="var(--ok)" />
          <h2>{t("始终排除")}</h2>
          <span className="muted">{t("不需要设置，这些项目都会保持原位")}</span>
        </div>
        {BUILTIN_EXCLUDES.map((x) => (
          <div key={x.title} className="list-row">
            <x.icon size={17} color="var(--muted)" />
            <div className="row-main">
              <div className="row-title">{x.title}</div>
              <div className="row-sub">{x.text}</div>
            </div>
          </div>
        ))}
      </div>
    </>
  );
}

type Schedule = NonNullable<NonNullable<RuleFile["settings"]>["schedule"]>;

/** 定时整理：关闭 / 每天 / 每周几，加上时间。 */
function ScheduleInput({ value, onChange }: { value: Schedule | undefined; onChange: (v: Schedule | undefined) => void }) {
  const mode = value?.every ?? "off";
  const at = value?.at ?? "09:00";
  const weekday = value?.weekday ?? 1;
  const days = [t("周一"), t("周二"), t("周三"), t("周四"), t("周五"), t("周六"), t("周日")];
  return (
    <span className="row" style={{ gap: 6 }}>
      <select
        className="input"
        style={{ width: "auto" }}
        value={mode}
        onChange={(e) => {
          const v = e.target.value;
          onChange(v === "off" ? undefined : v === "daily" ? { every: "daily", at } : { every: "weekly", at, weekday });
        }}
      >
        <option value="off">{t("关闭")}</option>
        <option value="daily">{t("每天")}</option>
        <option value="weekly">{t("每周")}</option>
      </select>
      {mode === "weekly" && (
        <select
          className="input"
          style={{ width: "auto" }}
          value={weekday}
          onChange={(e) => onChange({ every: "weekly", at, weekday: Number(e.target.value) })}
        >
          {days.map((d, i) => (
            <option key={d} value={i + 1}>
              {d}
            </option>
          ))}
        </select>
      )}
      {mode !== "off" && (
        <input
          type="time"
          className="input"
          style={{ width: "auto" }}
          value={at}
          onChange={(e) => e.target.value && onChange({ ...(value as Schedule), at: e.target.value })}
        />
      )}
    </span>
  );
}
