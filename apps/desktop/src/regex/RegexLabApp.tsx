import { useEffect, useMemo, useRef, useState } from "react";
import { getCurrentWindow } from "@tauri-apps/api/window";
import { open } from "@tauri-apps/plugin-dialog";
import { ArrowDown, ArrowUp, Check, Copy, FlaskConical, Plus, Sparkles, Upload, X } from "lucide-react";
import { api, errorText, type RegexSample, type RegexSession } from "../api";
import { Checkbox, Spinner } from "../components/ui";
import { t } from "../i18n";
import {
  buildPattern,
  DATE_PATTERN,
  friendlyRegexError,
  inferPatterns,
  partLabels,
  translateRegex,
  type Candidate,
  type Part,
} from "./model";

type Sample = { text: string; expected: boolean };
const initialParts: Part[] = [
  { kind: "digits", text: "", min: 7, max: null },
  { kind: "literal", text: "_", min: 1, max: 1 },
  { kind: "alnum", text: "", min: 1, max: null },
];

export default function RegexLabApp({ id }: { id: string }) {
  const [session, setSession] = useState<RegexSession | null>(null);
  const [pattern, setPattern] = useState("");
  const [cs, setCs] = useState(false);
  const [samples, setSamples] = useState<Sample[]>([]);
  const samplesRef = useRef(samples);
  samplesRef.current = samples;
  const [candidates, setCandidates] = useState<Candidate[]>([]);
  const [parts, setParts] = useState<Part[]>(initialParts);
  const [scope, setScope] = useState<"whole" | "start" | "anywhere">("whole");
  const [mode, setMode] = useState<"guide" | "code">("guide");
  const [hover, setHover] = useState(false);
  const [reading, setReading] = useState(0);
  const [working, setWorking] = useState(false);
  const [notice, setNotice] = useState("");
  const [failure, setFailure] = useState("");
  const [syntax, setSyntax] = useState("");
  const [results, setResults] = useState<RegexSample[]>([]);
  const [tested, setTested] = useState("");
  const [explanation, setExplanation] = useState("");
  const entries = samples.filter((s) => s.text.length > 0);
  const testKey = JSON.stringify([pattern, cs, entries.map((s) => s.text)]);
  const busy = tested !== testKey;
  const mismatch = busy || syntax ? 0 : entries.filter((s, i) => (results[i]?.range != null) !== s.expected).length;
  const translated = useMemo(() => translateRegex(pattern), [pattern]);
  useEffect(() => setExplanation(""), [pattern]);

  useEffect(() => {
    let active = true;
    api
      .regexSession(id)
      .then((context) => {
        if (!active) return;
        setSession(context);
        setPattern(context.pattern);
        setCs(context.caseSensitive);
        if (context.pattern) setMode("code");
      })
      .catch((e) => {
        if (active) setFailure(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [id]);
  useEffect(() => {
    let active = true;
    const timer = setTimeout(() => {
      api
        .testRegex(pattern, entries.map((s) => s.text).join("\n"), cs)
        .then((value) => {
          if (active) {
            setResults(value);
            setSyntax("");
          }
        })
        .catch((e) => {
          if (active) {
            setResults([]);
            setSyntax(errorText(e));
          }
        })
        .finally(() => {
          if (active) setTested(testKey);
        });
    }, 200);
    return () => {
      active = false;
      clearTimeout(timer);
    };
    // testKey includes every value consumed by the request.
  }, [testKey]);

  const replaceSamples = (next: Sample[]) => {
    samplesRef.current = next;
    setSamples(next);
  };
  const generate = (rows: Sample[]) => {
    const options = inferPatterns(rows.filter((s) => s.expected).map((s) => s.text));
    setCandidates(options);
    if (options.length) {
      setPattern(options[0].pattern);
      setMode("code");
      setNotice(t("已根据名称生成候选表达式，请检查匹配结果。"));
    } else setFailure(t("请先添加至少一个应该匹配的名称；样本过长时请减少数量。"));
  };
  const importPaths = async (paths: string[]) => {
    setReading((n) => n + 1);
    setFailure("");
    try {
      const incoming = await api.regexFileSamples(paths);
      const next = [...samplesRef.current];
      let skipped = incoming.ignored;
      for (const name of incoming.names) {
        if (next.some((s) => s.text === name)) continue;
        if (next.length >= 100 || new TextEncoder().encode([...next.map((s) => s.text), name].join("\n")).length > 65536) {
          ++skipped;
          continue;
        }
        next.push({ text: name, expected: true });
      }
      replaceSamples(next);
      generate(next);
      if (skipped) setNotice(t("已添加可读取的名称，跳过 {0} 个项目。", [skipped]));
      if (!incoming.names.length) setFailure(t("没有读取到文件名，请重新拖入。"));
    } catch (e) {
      setFailure(errorText(e));
    } finally {
      setReading((n) => n - 1);
    }
  };
  const importRef = useRef(importPaths);
  importRef.current = importPaths;
  useEffect(() => {
    if (!session) return;
    let active = true;
    let cache: string[] = [];
    let unlisten: (() => void) | undefined;
    void getCurrentWindow()
      .onDragDropEvent((event) => {
        if (!active) return;
        const drop = event.payload;
        if (drop.type === "enter") {
          cache = drop.paths;
          setHover(true);
        } else if (drop.type === "over") setHover(true);
        else if (drop.type === "leave") {
          cache = [];
          setHover(false);
        } else {
          setHover(false);
          const paths = drop.paths.length ? drop.paths : cache;
          cache = [];
          if (paths.length) void importRef.current(paths);
        }
      })
      .then((un) => {
        if (active) unlisten = un;
        else un();
      })
      .catch((e) => {
        if (active) setFailure(errorText(e));
      });
    return () => {
      active = false;
      unlisten?.();
    };
  }, [session]);
  const pick = async () => {
    try {
      const paths = await open({ multiple: true, directory: false, title: t("选择测试文件") });
      if (paths) await importPaths(Array.isArray(paths) ? paths : [paths]);
    } catch (e) {
      setFailure(errorText(e));
    }
  };
  const changePart = (index: number, patch: Partial<Part>) => setParts(parts.map((p, i) => (i === index ? { ...p, ...patch } : p)));
  const movePart = (i: number, step: number) => {
    const next = [...parts];
    [next[i], next[i + step]] = [next[i + step], next[i]];
    setParts(next);
  };
  if (!session) return <div className="regex-lab-loading">{failure ? <div role="alert">{failure}</div> : <Spinner size={22} />}</div>;
  return (
    <div className={`regex-lab ${hover ? "drag-active" : ""}`}>
      <header className="regex-lab-head">
        <FlaskConical size={23} />
        <div>
          <h1>{t("正则助手")}</h1>
          <p className="muted">{t("拖入文件生成规则，也可以按步骤组合；只读取名称，不移动文件。")}</p>
        </div>
      </header>
      <main className="regex-lab-body">
        <section className="card regex-lab-samples">
          <div className="card-head">
            <h2>{t("用文件名举例")}</h2>
            <span className="spacer" />
            <button className="btn sm" onClick={() => void pick()}>
              <Upload size={14} />
              {t("添加文件")}
            </button>
          </div>
          <div
            className={`regex-drop-zone ${hover ? "active" : ""}`}
            onClick={() => void pick()}
            role="button"
            tabIndex={0}
            onKeyDown={(e) => {
              if (e.key === "Enter" || e.key === " ") {
                e.preventDefault();
                void pick();
              }
            }}
          >
            <Upload size={22} />
            <strong>{hover ? t("松开以读取文件名") : t("拖入多个文件或文件夹")}</strong>
            <span>{t("自动生成候选表达式，不会修改原文件")}</span>
          </div>
          <div className="regex-sample-tools">
            <button
              className="btn sm"
              disabled={samples.length >= 100 || !!reading}
              onClick={() => replaceSamples([...samples, { text: "", expected: true }])}
            >
              <Plus size={14} />
              {t("手动添加名称")}
            </button>
            <button className="btn sm primary" disabled={!entries.some((s) => s.expected) || !!reading} onClick={() => generate(samples)}>
              <Sparkles size={14} />
              {t("根据样本生成")}
            </button>
          </div>
          <p className="muted regex-lab-tip">{t("勾选“应该匹配”；取消勾选可作为反例，检查是否误匹配。")}</p>
          <div className="regex-sample-list">
            {samples.map((sample, i) => {
              const index = samples.slice(0, i).filter((s) => s.text.length > 0).length;
              const result = sample.text ? results[index] : undefined;
              const correct = (result?.range != null) === sample.expected;
              return (
                <div className={`regex-sample-row ${!busy && !syntax && sample.text ? (correct ? "ok" : "bad") : ""}`} key={i}>
                  <div className="row">
                    <Checkbox
                      checked={sample.expected}
                      label={t("应该匹配")}
                      onChange={(expected) => replaceSamples(samples.map((s, n) => (n === i ? { ...s, expected } : s)))}
                    />
                    <input
                      className="input grow"
                      aria-label={t("示例名称 {0}", [i + 1])}
                      value={sample.text}
                      placeholder={t("输入文件名或文件夹名")}
                      onChange={(e) =>
                        replaceSamples(samples.map((s, n) => (n === i ? { ...s, text: e.target.value.replace(/[\r\n]/g, "") } : s)))
                      }
                    />
                    <button className="icon-btn" title={t("移除")} onClick={() => replaceSamples(samples.filter((_, n) => n !== i))}>
                      <X size={14} />
                    </button>
                  </div>
                  <div className="regex-sample-result">
                    {busy ? (
                      t("正在测试…")
                    ) : syntax || !sample.text ? (
                      "—"
                    ) : (
                      <>
                        <span>{correct ? t("符合预期") : t("与预期不符")}</span>
                        <code>
                          {result?.range ? (
                            <>
                              {sample.text.slice(0, result.range[0])}
                              <mark>{sample.text.slice(...result.range) || t("空匹配")}</mark>
                              {sample.text.slice(result.range[1])}
                            </>
                          ) : (
                            t("不匹配")
                          )}
                        </code>
                      </>
                    )}
                  </div>
                </div>
              );
            })}
            {!samples.length && <p className="muted regex-lab-tip">{t("建议准备不同日期、编号和项目名称的文件，减少误匹配。")}</p>}
          </div>
        </section>
        <section className="card regex-lab-editor">
          <div className="card-head">
            <h2>{t("生成与编辑")}</h2>
            <span className="spacer" />
            <button
              className="btn sm"
              onClick={() => {
                setPattern(DATE_PATTERN);
                setMode("code");
              }}
            >
              {t("日期开头示例")}
            </button>
          </div>
          <div className="regex-editor-inner">
            <div className="settings-tabs" role="tablist" aria-label={t("编辑方式")}>
              <button role="tab" aria-selected={mode === "guide"} onClick={() => setMode("guide")}>
                {t("按步骤组合")}
              </button>
              <button role="tab" aria-selected={mode === "code"} onClick={() => setMode("code")}>
                {t("直接编辑")}
              </button>
            </div>
            {mode === "guide" && (
              <div className="regex-builder">
                <p className="muted">{t("从上到下描述名称的组成，再点击生成表达式。")}</p>
                {parts.map((part, i) => (
                  <div className="regex-part" key={i}>
                    <span className="muted">{i + 1}</span>
                    <select
                      className="input"
                      aria-label={t("内容类型")}
                      value={part.kind}
                      onChange={(e) => changePart(i, { kind: e.target.value as Part["kind"] })}
                    >
                      {Object.entries(partLabels).map(([key, label]) => (
                        <option key={key} value={key}>
                          {label}
                        </option>
                      ))}
                    </select>
                    {part.kind === "literal" ? (
                      <input
                        className="input grow"
                        aria-label={t("固定文字")}
                        value={part.text}
                        onChange={(e) => changePart(i, { text: e.target.value })}
                      />
                    ) : (
                      <>
                        <select
                          className="input"
                          aria-label={t("重复方式")}
                          value={part.max === null ? "min" : part.max === part.min ? "exact" : "range"}
                          onChange={(e) =>
                            changePart(i, { max: e.target.value === "min" ? null : e.target.value === "exact" ? part.min : part.min + 5 })
                          }
                        >
                          <option value="exact">{t("恰好")}</option>
                          <option value="min">{t("至少")}</option>
                          <option value="range">{t("范围")}</option>
                        </select>
                        <input
                          className="input regex-count"
                          type="number"
                          min={1}
                          max={999}
                          aria-label={t("最少字符数")}
                          value={part.min}
                          onChange={(e) => {
                            const min = Math.max(1, Math.min(999, Number(e.target.value) || 1));
                            changePart(i, { min, max: part.max === null ? null : part.max === part.min ? min : Math.max(min, part.max) });
                          }}
                        />
                        {part.max !== null && part.max !== part.min && (
                          <input
                            className="input regex-count"
                            type="number"
                            min={part.min}
                            max={999}
                            aria-label={t("最多字符数")}
                            value={part.max}
                            onChange={(e) => changePart(i, { max: Math.max(part.min, Math.min(999, Number(e.target.value) || part.min)) })}
                          />
                        )}
                      </>
                    )}
                    <button className="icon-btn" title={t("上移")} disabled={!i} onClick={() => movePart(i, -1)}>
                      <ArrowUp size={13} />
                    </button>
                    <button className="icon-btn" title={t("下移")} disabled={i === parts.length - 1} onClick={() => movePart(i, 1)}>
                      <ArrowDown size={13} />
                    </button>
                    <button className="icon-btn" title={t("移除")} onClick={() => setParts(parts.filter((_, n) => n !== i))}>
                      <X size={13} />
                    </button>
                  </div>
                ))}
                <div className="row">
                  <button className="btn sm" onClick={() => setParts([...parts, { kind: "literal", text: "", min: 1, max: 1 }])}>
                    <Plus size={14} />
                    {t("添加一段")}
                  </button>
                  <select
                    className="input grow"
                    aria-label={t("匹配范围")}
                    value={scope}
                    onChange={(e) => setScope(e.target.value as typeof scope)}
                  >
                    <option value="whole">{t("整个名称符合")}</option>
                    <option value="start">{t("只要求开头符合")}</option>
                    <option value="anywhere">{t("名称中出现即可")}</option>
                  </select>
                  <button className="btn sm primary" disabled={!parts.length} onClick={() => setPattern(buildPattern(parts, scope))}>
                    {t("生成表达式")}
                  </button>
                </div>
              </div>
            )}
            {candidates.length > 0 && (
              <label className="regex-field">
                {t("从候选方案中选择")}
                <select
                  className="input"
                  value={candidates.some((c) => c.pattern === pattern) ? pattern : ""}
                  onChange={(e) => {
                    if (e.target.value) setPattern(e.target.value);
                  }}
                >
                  <option value="" disabled>
                    {t("当前为自定义表达式")}
                  </option>
                  {candidates.map((c) => (
                    <option key={c.pattern} value={c.pattern}>
                      {c.label}
                    </option>
                  ))}
                </select>
              </label>
            )}
            <label className="regex-field">
              {t("正则表达式")}
              <textarea
                className="input mono"
                rows={3}
                spellCheck={false}
                value={pattern}
                title={translated.map((p) => p.description).join(" → ")}
                onChange={(e) => setPattern(e.target.value)}
              />
            </label>
            <label className="row">
              <Checkbox checked={cs} onChange={setCs} label={t("区分大小写")} />
              <span>{t("区分大小写")}</span>
            </label>
            <div className="regex-translation">
              <strong>{t("这条表达式的意思")}</strong>
              <p className="muted">{t("悬停或聚焦片段查看解释；复杂表达式仍以测试结果为准。")}</p>
              <div className="regex-tokens">
                {translated.map((p, i) => (
                  <button
                    className="regex-token mono"
                    key={i}
                    title={p.description}
                    onMouseEnter={() => setExplanation(p.description)}
                    onFocus={() => setExplanation(p.description)}
                  >
                    {p.token}
                  </button>
                ))}
              </div>
              <p className="regex-explanation" aria-live="polite">
                {explanation || translated.map((p) => p.description).join(" → ") || t("生成或输入表达式后显示解释")}
              </p>
            </div>
            {!busy && syntax && (
              <div className="banner error" role="alert">
                <div>
                  {friendlyRegexError(syntax)}
                  <details>
                    <summary>{t("技术详情")}</summary>
                    <pre>{syntax}</pre>
                  </details>
                </div>
              </div>
            )}
            <p className="muted">{t("自动生成是根据样本推测，请用不应该匹配的名称检查边界。")}</p>
          </div>
        </section>
      </main>
      <footer className="regex-lab-footer">
        <div className="regex-lab-status" role="status">
          {reading || busy ? (
            <>
              <Spinner size={14} />
              {reading ? t("正在读取名称…") : t("正在测试…")}
            </>
          ) : failure ? (
            failure
          ) : syntax ? (
            t("请先修正表达式")
          ) : mismatch ? (
            t("{0} 个例子与预期不符", [mismatch])
          ) : (
            notice || t("所有例子符合预期")
          )}
        </div>
        <button
          className="btn"
          disabled={!pattern || busy || !!syntax}
          onClick={async () => {
            try {
              await navigator.clipboard.writeText(pattern);
              setNotice(t("表达式已复制"));
              setFailure("");
            } catch {
              setFailure(t("复制失败，请从输入框手动复制。"));
            }
          }}
        >
          <Copy size={15} />
          {t("复制表达式")}
        </button>
        {session.canApply && (
          <button
            className="btn primary"
            disabled={!pattern || busy || !!syntax || !!mismatch || !!reading || working}
            onClick={async () => {
              setWorking(true);
              try {
                await api.applyRegexLab(id, pattern, cs);
              } catch (e) {
                setFailure(errorText(e));
                setWorking(false);
              }
            }}
          >
            <Check size={15} />
            {t("应用到条件")}
          </button>
        )}
      </footer>
      {session.canApply && <div className="regex-lab-save-hint">{t("应用到条件后，仍需在规则窗口点击保存。")}</div>}
    </div>
  );
}
