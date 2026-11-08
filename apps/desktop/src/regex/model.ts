import { t } from "../i18n";
import datePrefix from "../../../../examples/date-prefix.regex?raw";

export type Part = {
  kind: "literal" | "digits" | "letters" | "han" | "alnum" | "space" | "any";
  text: string;
  min: number;
  max: number | null;
};
export type Candidate = { label: string; pattern: string };
export const DATE_PATTERN = datePrefix.trim();
export const escapeRegex = (text: string) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
export const partLabels: Record<Part["kind"], string> = {
  literal: t("固定文字"),
  digits: t("数字"),
  letters: t("英文字母"),
  han: t("中文汉字"),
  alnum: t("文字或数字"),
  space: t("空白字符"),
  any: t("任意内容"),
};
const atoms: Record<Part["kind"], string> = {
  literal: "",
  digits: "[0-9]",
  letters: "[a-zA-Z]",
  han: "\\p{Han}",
  alnum: "[\\p{L}\\p{N}]",
  space: "\\s",
  any: ".",
};
const count = (min: number, max: number | null) =>
  min === max ? (min === 1 ? "" : `{${min}}`) : max === null ? (min === 1 ? "+" : min === 0 ? "*" : `{${min},}`) : `{${min},${max}}`;
export function buildPattern(parts: Part[], scope: "whole" | "start" | "anywhere") {
  const body = parts.map((p) => (p.kind === "literal" ? escapeRegex(p.text) : atoms[p.kind] + count(p.min, p.max))).join("");
  return `${scope !== "anywhere" ? "^" : ""}${body}${scope === "whole" ? "$" : ""}`;
}

/** Local, conservative filename inference; candidates always remain reviewable. */
export function inferPatterns(input: string[]): Candidate[] {
  const names = [...new Set(input.filter(Boolean))].slice(0, 100);
  if (!names.length) return [];
  const candidates: Candidate[] = [];
  const add = (label: string, pattern: string) => {
    if (new TextEncoder().encode(pattern).length <= 4096 && !candidates.some((c) => c.pattern === pattern))
      candidates.push({ label, pattern });
  };
  if (names.every((name) => new RegExp(DATE_PATTERN, "u").test(name))) add(t("日期开头，后面必须有项目名称"), DATE_PATTERN);
  const split = names.map((name) => name.match(/[0-9]+|[\p{L}]+|[^0-9\p{L}]+/gu) ?? []);
  if (names.length > 1 && split.every((row) => row.length === split[0].length)) {
    const fragments = split[0].map((_, i) => {
      const values = split.map((row) => row[i]);
      if (values.every((v) => v === values[0])) return escapeRegex(values[0]);
      if (values.every((v) => /^[0-9]+$/.test(v))) {
        const lengths = values.map((v) => v.length);
        return "[0-9]" + count(Math.min(...lengths), Math.max(...lengths));
      }
      if (values.every((v) => /^[\p{L}]+$/u.test(v))) {
        // File extensions stay restricted to observed alternatives.
        if (i === split[0].length - 1 && split.every((row) => row[i - 1] === "."))
          return `(?:${[...new Set(values)].map(escapeRegex).join("|")})`;
        return "\\p{L}+";
      }
      return null;
    });
    if (fragments.every((f) => f !== null)) add(t("相同结构，变化部分按数字或文字匹配"), `^${fragments.join("")}$`);
  }
  if (names.length > 1) {
    const arrays = names.map((n) => Array.from(n));
    let first = 0;
    const shortest = Math.min(...arrays.map((a) => a.length));
    while (first < shortest && arrays.every((a) => a[first] === arrays[0][first])) ++first;
    let last = 0;
    while (last < shortest - first && arrays.every((a) => a[a.length - 1 - last] === arrays[0][arrays[0].length - 1 - last])) ++last;
    if (first || last) {
      const prefix = arrays[0].slice(0, first).join("");
      const suffix = last ? arrays[0].slice(-last).join("") : "";
      const middle = arrays.every((a) => a.length > first + last) ? ".+" : ".*";
      add(t("保留共同开头和结尾，中间内容可变化"), `^${escapeRegex(prefix)}${middle}${escapeRegex(suffix)}$`);
    }
  }
  add(t("仅匹配这些名称"), `^(?:${names.map(escapeRegex).join("|")})$`);
  return candidates;
}

export type Translation = { token: string; description: string };
export function translateRegex(pattern: string): Translation[] {
  const result: Translation[] = [];
  const tokens =
    pattern.match(
      /\\[pP]\{[^}]*\}|\\.|\[(?:\\.|[^\]\\])*\]|\(\?P?<[^>]*>|\(\?:|\(\?[=!]|\(\?<[=!]|\{\d+(?:,\d*)?\}[?+]?|[+*?][?+]?|[\^$().|]|[^\\[\]{}+*?^$().|]+|./gu,
    ) ?? [];
  for (const token of tokens) {
    let description: string;
    const fixed: Record<string, string> = {
      "^": t("从名称开头开始"),
      $: t("必须到名称结尾"),
      ".": t("一个任意字符（换行除外）"),
      "\\d": t("一位数字（包含 Unicode 数字）"),
      "[0-9]": t("一位数字 0–9"),
      "\\D": t("一个非数字字符"),
      "\\s": t("一个空白字符，例如空格"),
      "\\S": t("一个非空白字符"),
      "\\w": t("一个单词字符：文字、数字或下划线等"),
      "\\W": t("一个非单词字符"),
      "\\p{L}": t("一个文字字符，支持中文等语言"),
      "\\p{N}": t("一个数字字符"),
      "\\p{Han}": t("一个中文汉字"),
      "[a-zA-Z]": t("一个英文字母"),
      "[a-zA-Z0-9]": t("一个英文字母或数字"),
      "[\\p{L}\\p{N}]": t("一个文字或数字字符，支持中文"),
      "(": t("分组开始，并记住匹配内容"),
      "(?:": t("分组开始，不单独记住内容"),
      ")": t("分组结束"),
      "|": t("前后两种写法任选一种"),
      "+": t("前一项重复一次或多次"),
      "*": t("前一项可以没有，也可以重复多次"),
      "?": t("前一项可以没有，也可以出现一次"),
      "\\b": t("单词边界"),
      "\\B": t("不是单词边界"),
      "\\A": t("整个文本开头"),
      "\\z": t("整个文本结尾"),
    };
    const amount = token.match(/^\{(\d+)(,)?(\d*)\}([?+]?)$/);
    if (fixed[token]) description = fixed[token];
    else if (["(?=", "(?!", "(?<=", "(?<!"].includes(token)) description = t("预查语法：当前引擎不支持，请改写并测试");
    else if (amount) {
      description = !amount[2]
        ? t("前一项恰好重复 {0} 次", [amount[1]])
        : amount[3]
          ? t("前一项重复 {0}–{1} 次", [amount[1], amount[3]])
          : t("前一项至少重复 {0} 次", [amount[1]]);
      if (amount[4] === "?") description += t("；优先匹配较短内容");
      if (amount[4] === "+") description += t("；当前引擎不支持占有式重复");
    } else if (token.startsWith("["))
      description = token.startsWith("[^")
        ? t("一个不在此字符集合中的字符：{0}", [token.slice(2, -1)])
        : t("此字符集合中的一个字符：{0}", [token.slice(1, -1)]);
    else if (/^\(\?P?</.test(token)) description = t("命名分组开始：{0}", [token]);
    else if (/^[+*?]\?$/.test(token)) description = t("重复前一项，优先匹配较短内容");
    else if (/^\\[^A-Za-z0-9]$/.test(token)) description = t("固定文字：{0}", [token.slice(1)]);
    else if (token.startsWith("\\") || /^[{}\[\]]$/.test(token)) description = t("特殊语法：{0}；请结合测试结果确认", [token]);
    else description = t("固定文字：{0}", [token]);
    result.push({ token, description });
  }
  return result;
}

export function friendlyRegexError(message: string): string {
  if (message.includes("look-around"))
    return t("这里使用了预查语法 (?=…) 或 (?<=…)；当前引擎不支持。可改用直接匹配后续字符，再查看测试结果。");
  if (message.includes("backreferences")) return t("当前引擎不支持在表达式里引用前面的分组，例如 \\1。请改用明确的字符条件。");
  if (message.includes("unclosed group")) return t("左括号没有对应的右括号，请补上 )。");
  if (message.includes("unclosed character class")) return t("字符集合没有结束，请补上 ]。");
  return message;
}
