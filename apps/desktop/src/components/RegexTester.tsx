import { useEffect, useState } from "react";
import { CircleCheck, CircleX, FlaskConical, X } from "lucide-react";
import { api, errorText, type RegexSample } from "../api";
import { t } from "../i18n";
import { Checkbox, Spinner } from "./ui";
import datePrefix from "../../../../examples/date-prefix.regex?raw";

export default function RegexTester({
  pattern: initial,
  caseSensitive: initialCase,
  onApply,
  onClose,
}: {
  pattern: string;
  caseSensitive: boolean;
  onApply: (pattern: string, caseSensitive: boolean) => void;
  onClose: () => void;
}) {
  const [pattern, setPattern] = useState(initial);
  const [caseSensitive, setCaseSensitive] = useState(initialCase);
  const [samples, setSamples] = useState("");
  const [results, setResults] = useState<RegexSample[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [busy, setBusy] = useState(true);
  useEffect(() => {
    let active = true;
    setBusy(true);
    setError(null);
    const timer = setTimeout(() => {
      api
        .testRegex(pattern, samples, caseSensitive)
        .then((value) => {
          if (active) setResults(value);
        })
        .catch((e) => {
          if (active) {
            setError(errorText(e));
            setResults([]);
          }
        })
        .finally(() => {
          if (active) setBusy(false);
        });
    }, 200);
    return () => {
      active = false;
      clearTimeout(timer);
    };
  }, [pattern, samples, caseSensitive]);
  return (
    <section className="regex-tester" aria-label={t("正则调试")}>
      <div className="row">
        <FlaskConical size={16} />
        <strong>{t("正则调试")}</strong>
        <span className="spacer" />
        <button className="icon-btn" title={t("收起调试")} onClick={onClose}>
          <X size={15} />
        </button>
      </div>
      <label className="regex-field">
        {t("正则表达式")}
        <textarea className="input mono" rows={3} value={pattern} onChange={(e) => setPattern(e.target.value)} spellCheck={false} />
      </label>
      <div className="row regex-options">
        <Checkbox checked={caseSensitive} onChange={setCaseSensitive} label={t("区分大小写")} />
        <span>{t("区分大小写")}</span>
        <span className="spacer" />
        <button
          className="btn sm"
          onClick={() => {
            setPattern(datePrefix.trim());
            setSamples(
              "20160203 xxx\n1231 xxx\n26-12-12 xxx\n26.11.8 xxx\n2026-02-19_洞洞板\n1231-项目\n1111\n20260219\n2026-02-19_\n普通文件夹\n26.13.8 xxx",
            );
          }}
        >
          {t("日期开头示例")}
        </button>
      </div>
      <label className="regex-field">
        {t("测试文本（每行一个文件名）")}
        <textarea className="input mono" rows={5} value={samples} onChange={(e) => setSamples(e.target.value)} spellCheck={false} />
      </label>
      <p className="muted regex-help">
        {t("日期后需有项目名称，可用空格、_ 或 - 分隔")}
        <br />
        {t("与整理规则使用相同的正则语法；不支持前后查找和反向引用。日期示例检查月日范围，不校验闰年和每月天数。")}
      </p>
      <div aria-live="polite" className="regex-results">
        {busy ? (
          <Spinner size={16} />
        ) : error ? (
          <div className="banner error selectable">{error}</div>
        ) : (
          results.map((sample, i) => (
            <div className="regex-result" key={i}>
              <div className="row">
                {sample.range ? <CircleCheck size={15} color="var(--ok)" /> : <CircleX size={15} color="var(--muted)" />}
                <span className="regex-status">{sample.range ? t("匹配") : t("不匹配")}</span>
                <code className="selectable">
                  {sample.range ? (
                    <>
                      {sample.text.slice(0, sample.range[0])}
                      <mark>{sample.text.slice(...sample.range) || t("空匹配")}</mark>
                      {sample.text.slice(sample.range[1])}
                    </>
                  ) : (
                    sample.text
                  )}
                </code>
              </div>
              {sample.captures.length > 0 && (
                <div className="regex-captures">
                  {sample.captures.map((capture) => (
                    <code key={capture.index}>
                      {`{match.${capture.name ?? capture.index}}`} = {capture.value ?? "—"}
                    </code>
                  ))}
                </div>
              )}
            </div>
          ))
        )}
      </div>
      <div className="row">
        <span className="spacer" />
        <button
          className="btn primary sm"
          disabled={busy || !!error || !pattern.trim()}
          onClick={() => {
            onApply(pattern, caseSensitive);
            onClose();
          }}
        >
          {t("应用到条件")}
        </button>
      </div>
    </section>
  );
}
