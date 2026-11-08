import { describe, expect, it } from "vitest";
import { buildPattern, inferPatterns, translateRegex, friendlyRegexError } from "./model";
describe("可读正则生成", () => {
  it("固定内容转义，数字位数和项目名约束生效", () => {
    const pattern = buildPattern(
      [
        { kind: "digits", text: "", min: 7, max: null },
        { kind: "literal", text: "_[", min: 1, max: 1 },
        { kind: "alnum", text: "", min: 1, max: null },
      ],
      "whole",
    );
    const re = new RegExp(pattern, "u");
    expect(re.test("1234567_[洞洞板")).toBe(true);
    expect(re.test("123456_[abc")).toBe(false);
    expect(re.test("1234567_[")).toBe(false);
  });
  it("所有候选覆盖样本，日期排除无项目名称，精确方案不把特殊字符当语法", () => {
    for (const names of [
      ["IMG_001.png", "IMG_002.jpg"],
      ["2026-02-19_洞洞板", "1231 花架"],
      ["设计[1]+.psd", "other(2).psd"],
      ["中文001", "中文1234"],
    ]) {
      const candidates = inferPatterns(names);
      expect(candidates.length).toBeGreaterThan(0);
      for (const candidate of candidates) for (const name of names) expect(new RegExp(candidate.pattern, "u").test(name)).toBe(true);
      const exact = candidates.find((c) => c.label === "仅匹配这些名称")!;
      expect(new RegExp(exact.pattern, "u").test("something-else")).toBe(false);
    }
    const date = inferPatterns(["2026-02-19_洞洞板", "1231 花架"])[0];
    expect(new RegExp(date.pattern, "u").test("1111")).toBe(false);
  });
  it("悬停解释数量和字符集合，预查错误给出可读原因", () => {
    const parts = translateRegex("^\\d{7,}_[a-zA-Z0-9]");
    expect(parts.map((p) => p.token).join("")).toBe("^\\d{7,}_[a-zA-Z0-9]");
    expect(parts.find((p) => p.token === "{7,}")?.description).toBe("前一项至少重复 7 次");
    expect(friendlyRegexError("look-around is not supported")).toContain("预查");
  });
});
