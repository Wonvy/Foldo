// 界面翻译：按系统语言选语言、占位符替换、取值的专门译法。
import { afterEach, describe, expect, it, vi } from "vitest";

/** 用指定的系统语言重新加载翻译模块。 */
async function load(language: string) {
  Object.defineProperty(globalThis, "navigator", { value: { language, languages: [language] }, configurable: true });
  vi.resetModules();
  const m = await import("./i18n");
  await m.loadLocale();
  return m;
}

afterEach(() => {
  Object.defineProperty(globalThis, "navigator", { value: { language: "zh-CN", languages: ["zh-CN"] }, configurable: true });
});

describe("界面语言", () => {
  it("按系统语言选择", async () => {
    expect((await load("zh-CN")).lang).toBe("zh-CN");
    expect((await load("zh-Hant-TW")).lang).toBe("zh-TW");
    expect((await load("zh-HK")).lang).toBe("zh-TW");
    expect((await load("ja-JP")).lang).toBe("ja");
    expect((await load("en-GB")).lang).toBe("en");
    expect((await load("fr-FR")).lang).toBe("en");
  });

  it("简体中文显示原文，并替换占位符", async () => {
    const { t } = await load("zh-CN");
    expect(t("整理")).toBe("整理");
    expect(t("已选 {0} / {1} 个文件", [2, 5])).toBe("已选 2 / 5 个文件");
  });

  it("其他语言查译文，找不到时显示原文", async () => {
    const { t } = await load("en-US");
    expect(t("整理")).toBe("Tidy");
    expect(t("{0} 个文件待整理", [3])).toContain("3");
    expect(t("这段文字没有译文")).toBe("这段文字没有译文");
  });

  it("是 / 否 作为取值时有专门的译法", async () => {
    const { t, tv } = await load("en-US");
    expect(tv("是")).toBe("Yes");
    expect(tv("否")).toBe("No");
    // 作为运算符时仍是原来的译法
    expect(t("是")).not.toBe("Yes");
  });
});
