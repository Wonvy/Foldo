// 与 Rust 端（src-tauri/src/commands.rs）对应的类型和调用封装。
import { invoke } from "@tauri-apps/api/core";
import { t } from "./i18n";

// ---------- 配置（与 foldo-core 的 config.rs 对应） ----------

export type OneOrMany<T> = T | T[];

/** 一个条件：只有一个键的对象。键是字段名（如 `image.width`）或条件组（all / any / none / not）。 */
export type ConditionDef = Record<string, unknown>;

export type OnConflict = "rename" | "skip" | "overwrite";
export type DestDef = string | { to: string; on_conflict?: OnConflict };

export type ActionDef =
  | { move: DestDef }
  | { copy: DestDef }
  | { rename: DestDef }
  | { extract: DestDef }
  | { compress: DestDef }
  | { shortcut: DestDef }
  | "trash"
  | "keep";

/** 规则处理的对象：文件（默认）或整理文件夹第一层的子文件夹。 */
export type RuleTarget = "file" | "folder";

export type RuleDef = {
  name: string;
  enabled?: boolean;
  target?: RuleTarget;
  match?: "all" | "any";
  conditions?: ConditionDef[];
  when?: ConditionDef;
  except?: ConditionDef[];
  actions: ActionDef[];
};

export type FolderDef = {
  path: string;
  /** 连子文件夹里的文件一起整理 */
  recursive?: boolean;
  include_hidden?: boolean;
  /** 这个文件夹使用的规则名称；按本列表顺序匹配，第一条命中的生效 */
  rules?: string[];
};

export type UpdateInfo = {
  can_install: boolean;
  current: string;
  /** 有新版本时为新版本号 */
  version: string | null;
  notes: string | null;
  date: string | null;
};

/** 后台自动整理：关闭 / 发现文件时提醒 / 直接整理（移到回收站仍需确认）。 */
export type AutoMode = "off" | "notify" | "run";

export type WatchStatus = {
  mode: AutoMode;
  last_check: string | null;
  /** 待确认的文件数 */
  pending: number;
  last_batch: number | null;
  last_done: number;
  error: string | null;
};

export type RuleFile = {
  version: number;
  folders?: FolderDef[];
  protect?: string[];
  settings?: { buffer?: string; auto?: AutoMode; keep_empty_folders?: boolean; category_extensions?: Record<string, string[]> };
  rules?: RuleDef[];
};

export type ConfigView = {
  /** 用户主目录，用于把路径显示成 ~/… */
  home: string;
  path: string;
  display_path: string;
  exists: boolean;
  config: RuleFile | null;
  error: string | null;
};

export type Category = { id: string; name: string; aliases: string[]; extensions: string[] };

export type FieldType = "text" | "number" | "size" | "date" | "enum" | "bool" | "list";
export type FieldDef = {
  key: string;
  label: string;
  group: string;
  type: FieldType;
  unit: string | null;
  desc: string;
  options: string[];
  /** 适用于文件、文件夹还是两者 */
  applies: "file" | "folder" | "both";
};
export type VarDef = { key: string; label: string; group: string; desc: string };
export type FieldsView = { fields: FieldDef[]; vars: VarDef[] };
export type AttrView = { key: string; label: string; group: string; unit: string | null; value: string | null };
export type Explain = { ok: boolean; error?: string; children?: Explain[] };
export type TestView = {
  matched: boolean;
  conditions: Explain[];
  when: Explain | null;
  exceptions: Explain[];
  destination: string | null;
  reason: string | null;
};

// ---------- 扫描与执行 ----------

export type OpKind = "move" | "copy" | "rename" | "trash" | "extract" | "compress" | "shortcut";

export type OpView = {
  group: number;
  origin: string;
  origin_name: string;
  origin_display: string;
  rule: string;
  reason: string;
  category: string | null;
  kind: OpKind;
  src: string;
  src_display: string;
  dst: string | null;
  dst_display: string | null;
  dst_dir: string | null;
  dst_dir_display: string | null;
  note: string | null;
};

export type SkipKind =
  | "protected"
  | "credential"
  | "shortcut"
  | "system"
  | "hidden"
  | "partial"
  | "buffer"
  | "in_use"
  | "folder"
  | "no_match"
  | "conflict"
  | "invalid";

export type SkipView = {
  path: string;
  name: string;
  display: string;
  kind: SkipKind;
  label: string;
  reason: string;
  rule: string | null;
};

export type PlanView = {
  plan_id: number;
  groups: number;
  ops: OpView[];
  skipped: SkipView[];
  warnings: string[];
  scanned: number;
  matched: number;
  in_place: number;
};

export type Problem = { path: string; name: string; error: string };
export type ExecView = { batch_id: number; done: number; skipped: Problem[]; failed: Problem[] };

// ---------- 历史 ----------

export type Batch = {
  id: number;
  started_at: string;
  finished_at: string | null;
  source: string;
  status: string;
  op_count: number;
};

export type OpRecord = {
  id: number;
  seq: number;
  rule: string | null;
  kind: string;
  name: string;
  src: string;
  dst: string | null;
  src_display: string;
  dst_display: string | null;
  status: string;
  error: string | null;
};

export type UndoCheck = {
  kind: string;
  name: string;
  src_display: string;
  dst_display: string | null;
  outcome: "restore" | "in_trash" | "blocked";
  reason: string | null;
};

export type UndoView = { batch_id: number; restored: number; failed: Problem[]; in_trash: string[] };

export type RuleHit = { index: number; name: string; enabled: boolean; matched: boolean; in_scope: boolean };
export type DebugView = {
  path: string;
  name: string;
  display: string;
  category: string | null;
  excluded: string | null;
  rules: RuleHit[];
  winner: number | null;
  action: string | null;
  destination: string | null;
  reason: string | null;
};

export type FolderSuggestion = { path: string; display: string; label: string };

export const api = {
  getConfig: () => invoke<ConfigView>("get_config"),
  saveConfig: (config: RuleFile) => invoke<ConfigView>("save_config", { config }),
  addProtect: (entry: string) => invoke<ConfigView>("add_protect", { entry }),
  getYaml: () => invoke<string>("get_yaml"),
  saveYaml: (text: string) => invoke<ConfigView>("save_yaml", { text }),
  categories: () => invoke<Category[]>("categories"),
  suggestedFolders: () => invoke<FolderSuggestion[]>("suggested_folders"),
  scan: () => invoke<PlanView>("scan"),
  scanPaths: (paths: string[]) => invoke<PlanView>("scan_paths", { paths }),
  tryRule: (rule: RuleDef, paths: string[] | null) => invoke<PlanView>("try_rule", { rule, paths }),
  retarget: (planId: number, group: number, dir: string) =>
    invoke<PlanView>("retarget", { planId, group, dir }),
  executePlan: (planId: number, groups: number[]) =>
    invoke<ExecView>("execute_plan", { planId, groups }),
  history: (limit = 100) => invoke<Batch[]>("history", { limit }),
  batchOps: (id: number) => invoke<OpRecord[]>("batch_ops", { id }),
  undoPreview: (id: number) => invoke<UndoCheck[]>("undo_preview", { id }),
  undo: (id: number) => invoke<UndoView>("undo", { id }),
  fields: () => invoke<FieldsView>("fields"),
  inspect: (path: string) => invoke<AttrView[]>("inspect", { path }),
  testRule: (rule: RuleDef, path: string) => invoke<TestView>("test_rule", { rule, path }),
  debugFile: (path: string) => invoke<DebugView>("debug_file", { path }),
  toggleFloat: () => invoke<boolean>("toggle_float"),
  floatVisible: () => invoke<boolean>("float_visible"),
  floatResize: (width: number, height: number) => invoke<void>("float_resize", { width, height }),
  showMain: () => invoke<void>("show_main"),
  watchStatus: () => invoke<WatchStatus>("watch_status"),
  watchNow: () => invoke<void>("watch_now"),
  autostartEnabled: () => invoke<boolean>("autostart_enabled"),
  setAutostart: (enabled: boolean) => invoke<boolean>("set_autostart", { enabled }),
  exportRules: (names: string[], path: string) => invoke<number>("export_rules", { names, path }),
  readRulesFile: (path: string) => invoke<RuleDef[]>("read_rules_file", { path }),
  setLocale: (lang: string) => invoke<void>("set_locale", { lang }),
  checkUpdate: () => invoke<UpdateInfo>("check_update"),
  openReleases: () => invoke<void>("open_releases"),
  installUpdate: () => invoke<void>("install_update"),
  reveal: (path: string) => invoke<void>("reveal", { path }),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}

/** 路径最后一段（用作文件夹的短名称）。 */
export function baseName(p: string): string {
  const parts = p.split(/[\\/]/).filter(Boolean);
  return parts[parts.length - 1] ?? p;
}

export const STATUS_LABEL: Record<string, string> = {
  running: t("执行中断"),
  pending: t("未完成"),
  done: t("完成"),
  failed: t("失败"),
  skipped: t("已跳过"),
  partial: t("部分完成"),
  undone: t("已撤销"),
  partially_undone: t("部分撤销"),
};

export const KIND_LABEL: Record<string, string> = {
  move: t("移动"),
  copy: t("复制"),
  rename: t("改名"),
  trash: t("移到回收站"),
  extract: t("解压"),
  compress: t("压缩"),
  shortcut: t("快捷方式"),
  mkdir: t("新建文件夹"),
};
