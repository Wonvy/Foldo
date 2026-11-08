// 首次使用：选择要整理的文件夹（上手引导的第一步）。
import { ArrowRight, Folder, FolderPlus, Sparkles } from "lucide-react";
import { useEffect, useState } from "react";
import { api, baseName, errorText, type FolderSuggestion } from "../../api";
import { t } from "../../i18n";
import { pickFolder, useStore } from "../../store";
import { SetupSteps } from "../SetupGuide";
import { Checkbox, Spinner } from "../ui";

export default function Onboarding({ onDone }: { onDone: () => void }) {
  const { config, save } = useStore();
  const [suggestions, setSuggestions] = useState<FolderSuggestion[]>([]);
  const [chosen, setChosen] = useState<string[]>([]);
  const [extra, setExtra] = useState<string[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [saving, setSaving] = useState(false);

  useEffect(() => {
    api
      .suggestedFolders()
      .then((s) => {
        setSuggestions(s);
        // 默认选下载和桌面；文稿里多是整理好的东西，默认不选
        setChosen(s.filter((x) => x.key !== "documents").map((x) => x.path));
      })
      .catch(() => setSuggestions([]));
  }, []);

  const all = [
    ...suggestions.map((s) => ({ path: s.path, title: s.label, sub: s.display })),
    ...extra.map((p) => ({ path: p, title: baseName(p), sub: p })),
  ];

  const start = async () => {
    setSaving(true);
    setError(null);
    try {
      await save({
        ...config,
        version: 2,
        folders: chosen.map((path) => ({ path, rules: [] })),
        settings: config.settings ?? { buffer: "10m" },
      });
      onDone();
    } catch (e) {
      setError(errorText(e));
      setSaving(false);
    }
  };

  return (
    <div className="page">
      <div className="empty" style={{ maxWidth: 520 }}>
        <SetupSteps step={1} />
        <div className="art" style={{ marginTop: 18 }}>
          <Sparkles size={26} />
        </div>
        <h1>{t("选择要整理的文件夹")}</h1>
        <p className="muted">
          {t("第一步：选要整理的文件夹。接下来建规则，写明哪些文件或文件夹放到哪里；每次都先给你看预览，执行后也能撤销。")}
        </p>
        <div className="card" style={{ width: "100%", marginTop: 14, textAlign: "left" }}>
          {all.map((f) => (
            <label key={f.path} className="list-row clickable">
              <Checkbox
                checked={chosen.includes(f.path)}
                onChange={(on) => setChosen(on ? [...chosen, f.path] : chosen.filter((p) => p !== f.path))}
                label={f.title}
              />
              <Folder size={18} color="var(--accent)" />
              <div className="row-main">
                <div className="row-title">{f.title}</div>
                <div className="row-sub mono">{f.sub}</div>
              </div>
            </label>
          ))}
          <div className="list-row">
            <button
              className="btn ghost sm"
              onClick={async () => {
                const d = await pickFolder(t("选择要整理的文件夹"));
                if (d && !all.some((f) => f.path === d)) {
                  setExtra([...extra, d]);
                  setChosen([...chosen, d]);
                }
              }}
            >
              <FolderPlus size={15} /> {t("选择其他文件夹…")}
            </button>
          </div>
        </div>
        {error && <div className="banner error">{error}</div>}
        <button className="btn primary lg" style={{ marginTop: 10 }} disabled={chosen.length === 0 || saving} onClick={start}>
          {saving ? <Spinner size={15} /> : <ArrowRight size={16} />}
          {t("下一步")}
        </button>
      </div>
    </div>
  );
}
