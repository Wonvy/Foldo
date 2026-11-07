// 与 Rust 端（src-tauri/src/commands.rs）对应的类型和调用封装。
import { invoke } from "@tauri-apps/api/core";

export type RulesFile = {
  path: string;
  display_path: string;
  exists: boolean;
  text: string;
};

export type RuleInfo = {
  name: string;
  enabled: boolean;
  locations: string[];
  recursive: boolean;
  actions: number;
};

export type RulesSummary = { rules: RuleInfo[] };

export type OpKind = "move" | "copy" | "rename" | "trash";

export type OpView = {
  group: number;
  rule: string;
  kind: OpKind;
  src: string;
  dst: string | null;
  src_display: string;
  dst_display: string | null;
  note: string | null;
};

export type PlanView = {
  plan_id: number;
  groups: number;
  ops: OpView[];
  skipped: { rule: string; display: string; reason: string }[];
  warnings: string[];
  scanned: number;
  matched: number;
  in_place: number;
};

export type Failure = { path: string; error: string };

export type ExecView = { batch_id: number; done: number; failed: Failure[] };

export type UndoView = {
  batch_id: number;
  restored: number;
  failed: Failure[];
  in_trash: string[];
};

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
  src: string;
  dst: string | null;
  src_display: string;
  dst_display: string | null;
  status: string;
  error: string | null;
};

export const api = {
  loadRules: () => invoke<RulesFile>("load_rules"),
  createExampleRules: () => invoke<RulesFile>("create_example_rules"),
  validateRules: (text: string) => invoke<RulesSummary>("validate_rules", { text }),
  saveRules: (text: string) => invoke<RulesSummary>("save_rules", { text }),
  makePlan: (text: string, rule: string | null) => invoke<PlanView>("make_plan", { text, rule }),
  executePlan: (planId: number, groups: number[]) =>
    invoke<ExecView>("execute_plan", { planId, groups }),
  history: (limit = 100) => invoke<Batch[]>("history", { limit }),
  batchOps: (id: number) => invoke<OpRecord[]>("batch_ops", { id }),
  undo: (id: number) => invoke<UndoView>("undo", { id }),
  reveal: (path: string) => invoke<void>("reveal", { path }),
};

export function errorText(e: unknown): string {
  if (typeof e === "string") return e;
  if (e instanceof Error) return e.message;
  return JSON.stringify(e);
}

export const KIND_LABEL: Record<string, string> = {
  move: "移动",
  copy: "复制",
  rename: "改名",
  trash: "回收站",
  mkdir: "新建文件夹",
};

export const STATUS_LABEL: Record<string, string> = {
  running: "执行中断",
  pending: "未完成",
  done: "完成",
  failed: "失败",
  partial: "部分完成",
  undone: "已撤销",
  partially_undone: "部分撤销",
};
