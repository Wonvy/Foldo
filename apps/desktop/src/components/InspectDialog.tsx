// 查看一个文件的全部属性：写规则时参考字段名和取值。
import { Copy, TriangleAlert } from "lucide-react";
import { useEffect, useState } from "react";
import { api, baseName, errorText, type AttrView } from "../api";
import { Checkbox, Modal, Spinner } from "./ui";
import { t, tv } from "../i18n";

export default function InspectDialog({ path, onClose }: { path: string; onClose: () => void }) {
  const [attrs, setAttrs] = useState<AttrView[] | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [all, setAll] = useState(false);
  const [copied, setCopied] = useState<string | null>(null);

  useEffect(() => {
    api
      .inspect(path)
      .then(setAttrs)
      .catch((e) => setError(errorText(e)));
  }, [path]);

  const shown = (attrs ?? []).filter((a) => all || a.value !== null);
  const groups = [...new Set(shown.map((a) => a.group))];

  const copy = async (key: string) => {
    try {
      await navigator.clipboard.writeText(`{${key}}`);
      setCopied(key);
      setTimeout(() => setCopied(null), 1200);
    } catch {
      /* 剪贴板不可用 */
    }
  };

  return (
    <Modal title={t("文件属性")} subtitle={baseName(path)} onClose={onClose} width={640}>
      {error && (
        <div className="banner error">
          <TriangleAlert size={16} />
          {error}
        </div>
      )}
      {!attrs && !error && (
        <div className="row muted">
          <Spinner /> {t("正在读取…")}
        </div>
      )}
      {attrs && (
        <>
          <label className="row muted" style={{ fontSize: 12.5 }}>
            <Checkbox checked={all} onChange={setAll} label={t("显示读不到的字段")} />
            {t("显示读不到的字段")}
            <span className="spacer" />
            {t("点字段名可复制为变量")}
          </label>
          {groups.map((g) => (
            <div key={g} className="card">
              <div className="group-head">
                <span className="title">{t(g)}</span>
              </div>
              {shown
                .filter((a) => a.group === g)
                .map((a) => (
                  <div key={a.key} className="list-row" style={{ minHeight: 38 }}>
                    <div style={{ width: 150 }}>
                      <div>{t(a.label)}</div>
                      <button className="path-link mono muted" style={{ fontSize: 12 }} onClick={() => copy(a.key)} title={t("复制为变量")}>
                        {copied === a.key ? t("已复制") : a.key} <Copy size={11} />
                      </button>
                    </div>
                    <div className="row-main selectable" style={{ overflowWrap: "anywhere" }}>
                      {a.value === null ? (
                        <span className="muted">—</span>
                      ) : (
                        <span>
                          {tv(a.value)}
                          {a.unit && !a.unit.includes("/") && <span className="muted"> {a.unit}</span>}
                        </span>
                      )}
                    </div>
                  </div>
                ))}
            </div>
          ))}
        </>
      )}
      <div style={{ height: 8 }} />
    </Modal>
  );
}
