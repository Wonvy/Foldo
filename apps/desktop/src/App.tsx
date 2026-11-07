import { useEffect, useState } from "react";
import { api, errorText, type RulesFile } from "./api";
import RulesPage from "./pages/RulesPage";
import PreviewPage from "./pages/PreviewPage";
import HistoryPage from "./pages/HistoryPage";
import { Icon } from "./components";

type Page = "rules" | "preview" | "history";

const NAV: { id: Page; label: string; icon: Parameters<typeof Icon>[0]["name"] }[] = [
  { id: "rules", label: "规则", icon: "rules" },
  { id: "preview", label: "预览与执行", icon: "play" },
  { id: "history", label: "历史", icon: "history" },
];

export default function App() {
  const [page, setPage] = useState<Page>("rules");
  const [file, setFile] = useState<RulesFile | null>(null);
  const [loadError, setLoadError] = useState<string | null>(null);
  // 编辑器中的规则文本，以及最近一次保存到磁盘的版本
  const [text, setText] = useState("");
  const [savedText, setSavedText] = useState("");

  const applyFile = (f: RulesFile) => {
    setFile(f);
    setText(f.text);
    setSavedText(f.text);
  };

  useEffect(() => {
    api.loadRules().then(applyFile).catch((e) => setLoadError(errorText(e)));
  }, []);

  const dirty = text !== savedText;

  return (
    <div className="app">
      <nav className="sidebar">
        <div className="brand">
          <span className="brand-mark" aria-hidden>
            <Icon name="folder" />
          </span>
          Foldo
        </div>
        {NAV.map((n) => (
          <button
            key={n.id}
            className={`nav-item ${page === n.id ? "active" : ""}`}
            onClick={() => setPage(n.id)}
          >
            <Icon name={n.icon} />
            <span>{n.label}</span>
            {n.id === "rules" && dirty && <span className="dot" title="有未保存的修改" />}
          </button>
        ))}
      </nav>

      <main className="content">
        {loadError && <div className="banner error">读取规则文件失败：{loadError}</div>}
        {!file && !loadError && <div className="empty">正在加载…</div>}
        {file && (
          <>
            <div hidden={page !== "rules"} className="page">
              <RulesPage
                file={file}
                text={text}
                dirty={dirty}
                onChange={setText}
                onSaved={(t) => setSavedText(t)}
                onCreated={applyFile}
                onRevert={() => setText(savedText)}
              />
            </div>
            <div hidden={page !== "preview"} className="page">
              <PreviewPage text={text} dirty={dirty} active={page === "preview"} />
            </div>
            <div hidden={page !== "history"} className="page">
              <HistoryPage active={page === "history"} />
            </div>
          </>
        )}
      </main>
    </div>
  );
}
