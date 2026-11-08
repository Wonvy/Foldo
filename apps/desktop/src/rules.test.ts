// 规则编辑模型与配置的互相转换、描述文字、文件夹引用的维护。
import { describe, expect, it } from "vitest";
import type { FieldDef, RuleDef, RuleFile } from "./api";
import {
  defToNode,
  describeConditions,
  emptyForm,
  fieldFits,
  formToRule,
  foldersUsing,
  nodeToDef,
  pruneFolderRules,
  renameInFolders,
  switchProfile,
  ruleToForm,
  shortPath,
  type FieldMap,
} from "./rules";

const F = (key: string, label: string, type: FieldDef["type"], applies: FieldDef["applies"], options: string[] = []): FieldDef => ({
  key,
  label,
  group: "基本",
  type,
  unit: null,
  desc: "",
  options,
  applies,
});

const FIELDS: FieldMap = new Map(
  [
    F("name", "文件名", "text", "both"),
    F("ext", "扩展名", "text", "file"),
    F("category", "类型", "enum", "file", ["图片", "文档"]),
    F("size", "大小", "size", "both"),
    F("modified", "修改时间", "date", "both"),
    F("hidden", "隐藏", "bool", "both"),
    F("folder.content", "内容类型", "enum", "folder", ["图片", "混合"]),
    F("folder.names", "里面的项目名", "list", "folder"),
  ].map((f) => [f.key, f]),
);

/** 表单 → 规则，断言没有错误。 */
function build(form: ReturnType<typeof ruleToForm>): RuleDef {
  const r = formToRule(form, FIELDS);
  if ("error" in r) throw new Error(r.error);
  return r.rule;
}

describe("规则 ↔ 表单", () => {
  it("条件树、例外和各种动作往返不变", () => {
    const rule: RuleDef = {
      name: "相册",
      when: {
        all: [
          { category: { eq: "图片" } },
          { not: { name: { contains: "草稿" } } },
          { any: [{ size: { gt: "10MB" } }, { modified: { older_than: "30d" } }] },
        ],
      },
      except: [{ name: { glob: "*.tmp" } }],
      actions: [
        { rename: "{name}" },
        { move: { to: "照片/", on_conflict: "skip" } },
        { compress: "{stem}.zip" },
        { shortcut: "~/Desktop/" },
      ],
    };
    expect(build(ruleToForm(rule, FIELDS))).toEqual(rule);
  });

  it("处理文件夹的规则保留 target", () => {
    const rule: RuleDef = {
      name: "照片集",
      target: "folder",
      when: { all: [{ "folder.content": { eq: "图片" } }, { "folder.names": { not_contains: "草稿" } }] },
      actions: [{ move: "相册/" }],
    };
    const form = ruleToForm(rule, FIELDS);
    expect(form.target).toBe("folder");
    expect(build(form)).toEqual(rule);
  });

  it("旧写法 conditions + match 转成条件树", () => {
    const form = ruleToForm(
      { name: "x", match: "any", conditions: [{ extension: ["pdf", "docx"] }, { name: "*发票*" }], actions: ["trash"] },
      FIELDS,
    );
    expect(form.root.mode).toBe("any");
    expect(build(form).when).toEqual({ any: [{ ext: { in: ["pdf", "docx"] } }, { name: { glob: "*发票*" } }] });
  });

  it("停用的规则写出 enabled: false", () => {
    const form = { ...emptyForm(FIELDS), name: "停用的", enabled: false };
    form.actions = [{ kind: "keep", to: "", onConflict: "rename" }];
    expect(build(form)).toMatchObject({ name: "停用的", enabled: false, actions: ["keep"] });
  });

  it("文件夹规则的默认条件是“内容类型”", () => {
    const form = emptyForm(FIELDS, "folder");
    expect(form.target).toBe("folder");
    expect(form.root.children[0]).toMatchObject({ field: "folder.content", op: "eq", value: "图片" });
  });
});

describe("表单校验", () => {
  const base = () => ({ ...emptyForm(FIELDS), name: "规则" });

  it("缺少名称", () => {
    expect(formToRule({ ...base(), name: "  " }, FIELDS)).toEqual({ error: "请填写规则名称" });
  });
  it("缺少目标文件夹", () => {
    expect(formToRule(base(), FIELDS)).toEqual({ error: "请选择目标文件夹" });
  });
  it("回收站只能放在最后", () => {
    const f = base();
    f.actions = [
      { kind: "trash", to: "", onConflict: "rename" },
      { kind: "move", to: "a/", onConflict: "rename" },
    ];
    expect(formToRule(f, FIELDS)).toEqual({ error: "“移到回收站”和“保持原位”只能放在最后" });
  });
  it("条件值不能为空", () => {
    const f = base();
    f.actions = [{ kind: "move", to: "a/", onConflict: "rename" }];
    f.root.children = [defToNode({ name: { contains: "" } }, FIELDS)];
    expect(formToRule(f, FIELDS)).toEqual({ error: "请填写“文件名”的值" });
  });
});

describe("条件节点", () => {
  it("同一字段多个运算符拆成“全部满足”的小组", () => {
    const n = defToNode({ size: { gt: "1MB", lt: "1GB" } }, FIELDS);
    expect(n.kind).toBe("group");
    expect(nodeToDef(n)).toEqual({ all: [{ size: { gt: "1MB" } }, { size: { lt: "1GB" } }] });
  });
  it("运算符别名统一成标准写法", () => {
    expect(nodeToDef(defToNode({ size: { ">=": "1MB" } }, FIELDS))).toEqual({ size: { gte: "1MB" } });
    expect(nodeToDef(defToNode({ name: { regex: "^IMG" } }, FIELDS))).toEqual({ name: { matches: "^IMG" } });
  });
  it("区分大小写的标记保留", () => {
    expect(nodeToDef(defToNode({ name: { contains: "A", case_sensitive: true } }, FIELDS))).toEqual({
      name: { contains: "A", case_sensitive: true },
    });
  });
});

describe("描述文字", () => {
  it("把条件写成一句话", () => {
    const rule: RuleDef = {
      name: "x",
      when: { all: [{ category: { eq: "图片" } }, { any: [{ hidden: { eq: true } }, { modified: { older_than: "30d" } }] }] },
      actions: ["trash"],
    };
    expect(describeConditions(rule, FIELDS)).toBe("类型 是 图片 且 （隐藏 是 或 修改时间在 30 天之前）");
  });
  it("没有条件时是“所有文件”", () => {
    expect(describeConditions({ name: "x", actions: ["keep"] }, FIELDS)).toBe("所有文件");
  });
});

describe("字段适用范围", () => {
  it("文件夹规则只能用文件夹和通用字段", () => {
    expect(fieldFits(FIELDS.get("folder.names"), "folder")).toBe(true);
    expect(fieldFits(FIELDS.get("name"), "folder")).toBe(true);
    expect(fieldFits(FIELDS.get("ext"), "folder")).toBe(false);
    expect(fieldFits(FIELDS.get("folder.names"), "file")).toBe(false);
  });
});

describe("文件夹里的规则引用", () => {
  const file: RuleFile = {
    version: 2,
    folders: [
      { path: "/a", rules: ["甲", "乙"] },
      { path: "/b", rules: ["乙"] },
    ],
    rules: [{ name: "甲", actions: ["keep"] }],
  };
  it("删除规则后去掉引用", () => {
    expect(pruneFolderRules(file).folders?.map((f) => f.rules)).toEqual([["甲"], []]);
  });
  it("改名后同步引用", () => {
    expect(renameInFolders(file, "乙", "丙").folders?.map((f) => f.rules)).toEqual([["甲", "丙"], ["丙"]]);
  });
  it("查使用某条规则的文件夹", () => {
    expect(foldersUsing(file, "乙").map((f) => f.path)).toEqual(["/a", "/b"]);
  });
});

describe("路径显示", () => {
  it("长路径只留最后两级", () => {
    expect(shortPath("/Users/me/Documents/工作/2026/发票/")).toBe("…/2026/发票");
    expect(shortPath("照片/")).toBe("照片");
  });
});

describe("配置方案", () => {
  const file: RuleFile = {
    version: 2,
    folders: [{ path: "/work", rules: ["甲"] }],
    profile: "工作",
    profiles: [
      { name: "工作", folders: [] },
      { name: "个人", folders: [{ path: "/home", rules: ["甲", "乙"] }] },
    ],
    rules: [
      { name: "甲", actions: ["keep"] },
      { name: "乙", actions: ["keep"] },
    ],
  };
  it("切换时存回当前方案、换上目标方案", () => {
    const next = switchProfile(file, "个人", "默认方案");
    expect(next.profile).toBe("个人");
    expect(next.folders?.[0].path).toBe("/home");
    expect(next.profiles?.find((p) => p.name === "工作")?.folders[0].path).toBe("/work");
  });
  it("没有当前方案时存成默认方案", () => {
    const next = switchProfile({ ...file, profile: undefined }, "个人", "默认方案");
    expect(next.profiles?.[0].name).toBe("默认方案");
  });
  it("改名和删除规则也同步到其他方案", () => {
    expect(renameInFolders(file, "乙", "丙").profiles?.[1].folders[0].rules).toEqual(["甲", "丙"]);
    const pruned = pruneFolderRules({ ...file, rules: [{ name: "甲", actions: ["keep"] }] });
    expect(pruned.profiles?.[1].folders[0].rules).toEqual(["甲"]);
  });
});
