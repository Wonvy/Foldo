// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlanView } from "../api";
import FloatApp from "./FloatApp";

const mocks = vi.hoisted(() => ({
  drop: undefined as undefined | ((event: { payload: { type: string; paths: string[] } }) => void),
  scan: vi.fn(),
  execute: vi.fn(),
  close: vi.fn(),
  show: vi.fn(),
  drag: vi.fn(),
  emit: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: async (callback: typeof mocks.drop) => {
      mocks.drop = callback;
      return () => {};
    },
    startDragging: mocks.drag,
    setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({ emitTo: mocks.emit, listen: vi.fn().mockResolvedValue(() => {}) }));
vi.mock("../api", async (original) => ({
  ...(await original<typeof import("../api")>()),
  api: {
    scanPaths: mocks.scan,
    executePlan: mocks.execute,
    toggleFloat: mocks.close,
    showMain: mocks.show,
    floatResize: vi.fn().mockResolvedValue(undefined),
    undo: vi.fn(),
    reveal: vi.fn(),
  },
}));

function plan(paths: string[]): PlanView {
  const result: PlanView = { plan_id: 10, groups: 0, ops: [], skipped: [], warnings: [], scanned: paths.length, matched: 0, in_place: 0 };
  for (const [group, path] of [...paths].sort().entries()) {
    const name = path.split(/[\\/]/).pop()!;
    if (name.endsWith(".unknown")) {
      result.skipped.push({ path, name, display: path, kind: "no_match", label: "没有匹配的规则", reason: "没有匹配的规则", rule: null });
    } else {
      result.ops.push({
        group,
        origin: path,
        origin_name: name,
        origin_display: path,
        rule: "整理规则",
        reason: "匹配规则",
        category: name.endsWith(".pdf") ? "文档" : "图片",
        kind: "move",
        src: path,
        src_display: path,
        dst: `D:/out/${name}`,
        dst_display: `D:/out/${name}`,
        dst_dir: "D:/out",
        dst_dir_display: "D:/out",
        note: null,
      });
    }
  }
  result.groups = result.ops.length;
  return result;
}

async function drop(paths: string[]) {
  await waitFor(() => expect(mocks.drop).toBeTypeOf("function"));
  await act(async () => mocks.drop!({ payload: { type: "drop", paths } }));
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  mocks.scan.mockImplementation(async (paths: string[]) => plan(paths));
  mocks.close.mockResolvedValue(false);
  mocks.show.mockResolvedValue(undefined);
  mocks.emit.mockResolvedValue(undefined);
  mocks.drag.mockResolvedValue(undefined);
  mocks.execute.mockResolvedValue({ batch_id: 1, done: 1, skipped: [], failed: [] });
});
afterEach(cleanup);

describe("悬浮窗文件列表", () => {
  it("展开后继续拖入会追加、去重，并保留已经排除的文件", async () => {
    render(<FloatApp />);
    await drop(["D:/files/b.pdf"]);
    fireEvent.click(screen.getByRole("checkbox", { name: "b.pdf" }));
    await drop(["D:/files/a.png", "d:\\files\\B.pdf"]);
    expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/b.pdf", "D:/files/a.png"]);
    expect((screen.getByRole("checkbox", { name: "b.pdf" }) as HTMLInputElement).checked).toBe(false);
    fireEvent.click(screen.getByRole("button", { name: "整理 1 个文件" }));
    await waitFor(() => expect(mocks.execute).toHaveBeenCalledWith(10, [0]));
  });

  it("按类型整组排除和折叠不会执行被排除的文件", async () => {
    render(<FloatApp />);
    await drop(["D:/files/a.pdf", "D:/files/b.pdf", "D:/files/c.png"]);
    fireEvent.click(screen.getByRole("checkbox", { name: "文档" }));
    fireEvent.click(screen.getByRole("button", { name: "文档 0/2" }));
    expect(screen.queryByRole("checkbox", { name: "a.pdf" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "整理 1 个文件" }));
    await waitFor(() => expect(mocks.execute).toHaveBeenCalledWith(10, [2]));
  });

  it("切换扩展名和列表视图保留排除状态", async () => {
    render(<FloatApp />);
    await drop(["D:/files/a.pdf", "D:/files/b.pdf", "D:/files/c.png"]);
    fireEvent.change(screen.getByRole("combobox", { name: "文件视图" }), { target: { value: "extension" } });
    fireEvent.click(screen.getByRole("checkbox", { name: ".pdf" }));
    fireEvent.change(screen.getByRole("combobox", { name: "文件视图" }), { target: { value: "none" } });
    expect((screen.getByRole("checkbox", { name: "a.pdf" }) as HTMLInputElement).checked).toBe(false);
    expect(screen.getByRole("button", { name: "整理 1 个文件" })).toBeTruthy();
  });

  it("清空会放弃当前预览，迟到的扫描结果不会重新填回列表", async () => {
    let resolve!: (plan: PlanView) => void;
    mocks.scan.mockImplementationOnce(
      () =>
        new Promise<PlanView>((done) => {
          resolve = done;
        }),
    );
    render(<FloatApp />);
    await drop(["D:/files/a.pdf"]);
    fireEvent.click(screen.getByTitle("清空列表"));
    await act(async () => resolve(plan(["D:/files/a.pdf"])));
    expect(screen.queryByRole("checkbox")).toBeNull();
    expect(screen.getByText("拖入文件或文件夹")).toBeTruthy();
    expect(mocks.execute).not.toHaveBeenCalled();
    await drop(["D:/files/c.png"]);
    expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/c.png"]);
  });

  it("右上角关闭按钮不触发窗口拖动", async () => {
    render(<FloatApp />);
    await drop(["D:/files/a.pdf"]);
    const close = screen.getByTitle("关闭悬浮窗");
    fireEvent.mouseDown(close, { button: 0, detail: 1 });
    fireEvent.click(close);
    expect(mocks.drag).not.toHaveBeenCalled();
    expect(mocks.close).toHaveBeenCalledTimes(1);
  });

  it("整理完成后收起面板，再拖入只处理新一批文件", async () => {
    render(<FloatApp />);
    await drop(["D:/files/a.pdf"]);
    fireEvent.click(screen.getByRole("button", { name: "整理 1 个文件" }));
    await screen.findByText("已整理 1 项");
    fireEvent.click(screen.getByTitle("收起面板"));
    await drop(["D:/files/b.pdf"]);
    expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/b.pdf"]);
    expect(screen.queryByRole("checkbox", { name: "a.pdf" })).toBeNull();
  });

  it("未匹配条目可打开添加规则入口并带上测试文件", async () => {
    render(<FloatApp />);
    await drop(["D:/files/a.unknown"]);
    fireEvent.click(screen.getByTitle("为此文件添加规则"));
    await waitFor(() => expect(mocks.emit).toHaveBeenCalledWith("main", "float-new-rule", { path: "D:/files/a.unknown" }));
    expect(mocks.show).toHaveBeenCalled();
  });
});
