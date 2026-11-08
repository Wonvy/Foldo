import { File, Folder, FolderPlus, KeyRound, Link, Lock, ShieldCheck, Trash, TriangleAlert, X } from "lucide-react";
import { useState } from "react";
import { api, baseName, errorText, type RuleFile } from "../api";
import { Segmented, Switch } from "../components/ui";
import { pickFile, pickFolder, useStore } from "../store";

const BUFFERS = [
  { value: "none", label: "不设" },
  { value: "10m", label: "10 分钟" },
  { value: "1h", label: "1 小时" },
  { value: "1d", label: "1 天" },
];

const BUILTIN_EXCLUDES = [
  { icon: Folder, title: "已有文件夹", text: "作为整体保留，不拆分内部文件（代码工程、模型装配等保持原有结构）" },
  { icon: KeyRound, title: "凭证文件", text: "私钥、证书、密码库、.env 等默认不移动" },
  { icon: Link, title: "快捷方式", text: ".lnk、.url、.webloc 等保留在原位，也不会影响它们指向的文件" },
  { icon: ShieldCheck, title: "系统与隐藏文件", text: ".DS_Store、Thumbs.db、desktop.ini 以及隐藏文件" },
  { icon: TriangleAlert, title: "未完成或正在使用", text: "下载中的临时文件、Office 打开时的锁文件、被其他程序占用的文件" },
];

export default function ScopePage() {
  const { config, save, setView } = useStore();
  const folders = config.folders ?? [];
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

  const addFolder = async () => {
    const dir = await pickFolder("添加要整理的文件夹");
    if (dir && !folders.some((f) => f.path === dir)) {
      await update({ ...config, folders: [...folders, { path: dir }] });
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
    <div className="page">
      <div className="page-head">
        <div>
          <h1>范围与保护</h1>
          <p className="sub">决定整理哪里、哪些东西永远不动。</p>
        </div>
      </div>

      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          {error}
        </div>
      )}

      <div className="card">
        <div className="card-head">
          <h2>整理文件夹</h2>
          <span className="muted">规则默认在这些文件夹里生效，只整理第一层的散落文件</span>
          <span className="spacer" />
          <button className="btn sm" onClick={addFolder}>
            <FolderPlus size={15} /> 添加
          </button>
        </div>
        {folders.length === 0 && <div className="list-row muted">还没有整理文件夹。</div>}
        {folders.map((f, i) => (
          <div key={f.path} className="list-row">
            <Folder size={18} color="var(--accent)" />
            <div className="row-main">
              <div className="row-title">{baseName(f.path)}</div>
              <div className="row-sub mono ellipsis">{f.path}</div>
            </div>
            <button
              className="icon-btn"
              aria-label="移除"
              onClick={() => update({ ...config, folders: folders.filter((_, j) => j !== i) })}
            >
              <X size={16} />
            </button>
          </div>
        ))}
      </div>

      <div className="card">
        <div className="card-head">
          <Lock size={16} color="var(--muted)" />
          <h2>固定保留</h2>
          <span className="muted">这些文件或文件夹永远不会被移动</span>
          <span className="spacer" />
          <button className="btn sm" onClick={async () => addProtect(await pickFile("选择要固定保留的文件"))}>
            <File size={14} /> 文件…
          </button>
          <button className="btn sm" onClick={async () => addProtect(await pickFolder("选择要固定保留的文件夹"))}>
            <Folder size={14} /> 文件夹…
          </button>
        </div>
        {protect.map((p, i) => {
          const isPattern = !(p.startsWith("~") || p.includes("/") || p.includes("\\"));
          return (
            <div key={p} className="list-row">
              {isPattern ? <span className="chip">通配符</span> : <Lock size={16} color="var(--muted)" />}
              <div className="row-main">
                <div className="row-title mono ellipsis">{isPattern ? p : baseName(p)}</div>
                {!isPattern && <div className="row-sub mono ellipsis">{p}</div>}
              </div>
              <button
                className="icon-btn"
                aria-label="移除"
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
            placeholder="按文件名保留，如 *.lic 或 *合同*"
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
            添加
          </button>
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <h2>新文件缓冲期</h2>
          <span className="muted">刚下载或刚修改的文件先不整理，避免打断正在进行的工作</span>
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
      </div>

      <div className="card">
        <div className="card-body row">
          <div style={{ flex: 1 }}>
            <div>整理后删除变空的文件夹</div>
            <div className="muted" style={{ fontSize: 12.5 }}>只删除因为文件被移走而变空的文件夹；撤销时会重建</div>
          </div>
          <Switch
            label="整理后删除变空的文件夹"
            on={!config.settings?.keep_empty_folders}
            onChange={(on) =>
              update({
                ...config,
                settings: { ...config.settings, keep_empty_folders: on ? undefined : true },
              })
            }
          />
        </div>
      </div>

      <div className="card">
        <div className="card-head">
          <ShieldCheck size={16} color="var(--ok)" />
          <h2>始终排除</h2>
          <span className="muted">不需要设置，这些项目都会保持原位</span>
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
    </div>
  );
}
