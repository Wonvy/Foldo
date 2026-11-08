// 界面语言：简体中文（原文）、English、繁體中文、日本語。
//
// 界面里的文字以简体中文写在代码中，用 t("原文") 包起来；其他语言在仓库根目录的 locales/*.json 里按原文查找译文
// （与后台、命令行共用一份），
// 找不到时显示原文。原文里的 {0}、{1}… 由参数依次替换。
// 选择保存在本机（localStorage）；默认跟随系统语言。切换语言后重新加载窗口，所有文字一次性换好。
// 只加载当前语言的译文（loadLocale），要在加载界面之前完成，因为有些文字在模块加载时就会翻译。

export type Lang = "zh-CN" | "en" | "zh-TW" | "ja";
export type LangPref = Lang | "system";

export const LANGS: { value: Lang; label: string }[] = [
  { value: "zh-CN", label: "简体中文" },
  { value: "en", label: "English" },
  { value: "zh-TW", label: "繁體中文" },
  { value: "ja", label: "日本語" },
];

const KEY = "foldo.lang";

type Dict = Record<string, string>;
const LOADERS: Record<Lang, () => Promise<Dict>> = {
  "zh-CN": async () => ({}),
  en: () => import("../../../locales/en.json").then((m) => m.default),
  "zh-TW": () => import("../../../locales/zh-TW.json").then((m) => m.default),
  ja: () => import("../../../locales/ja.json").then((m) => m.default),
};

/** 按系统语言选一个支持的语言。 */
export function systemLang(): Lang {
  const list = typeof navigator !== "undefined" ? (navigator.languages?.length ? navigator.languages : [navigator.language]) : [];
  for (const raw of list) {
    const l = (raw ?? "").toLowerCase();
    if (l.startsWith("zh")) {
      return /hant|tw|hk|mo/.test(l) ? "zh-TW" : "zh-CN";
    }
    if (l.startsWith("ja")) return "ja";
    if (l.startsWith("en")) return "en";
  }
  return "en";
}

export function loadLangPref(): LangPref {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "system" || LANGS.some((l) => l.value === v)) return v as LangPref;
  } catch {
    /* 存储不可用 */
  }
  return "system";
}

/** 保存语言选择并重新加载窗口。 */
export function setLangPref(pref: LangPref) {
  try {
    localStorage.setItem(KEY, pref);
  } catch {
    /* 存储不可用 */
  }
  location.reload();
}

const pref = loadLangPref();
/** 当前界面语言。 */
export const lang: Lang = pref === "system" ? systemLang() : pref;
let dict: Dict = {};

/** 加载当前语言的译文。界面模块要在它完成之后再加载。 */
export async function loadLocale(): Promise<void> {
  try {
    dict = await LOADERS[lang]();
  } catch {
    dict = {};
  }
}

if (typeof document !== "undefined") document.documentElement.lang = lang;

/**
 * 翻译“取值”：和界面文字原文相同、但在其他语言里译法不同的值（如是/否字段的“是”“否”，
 * 英文是 Yes / No 而不是运算符的 Is / Is not）。译文写在 `原文#value` 下，没有时按普通文字翻译。
 */
export function tv(text: string): string {
  return dict[`${text}#value`] ?? t(text);
}

/** 翻译一段界面文字。`args` 依次替换原文里的 {0}、{1}… */
export function t(text: string, args?: (string | number)[]): string {
  let s = dict[text] ?? text;
  if (args) s = s.replace(/\{(\d+)\}/g, (m, i) => (args[Number(i)] !== undefined ? String(args[Number(i)]) : m));
  return s;
}
