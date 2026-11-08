// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import type { PlanView } from "../api";
import FloatApp from "./FloatApp";

const mocks = vi.hoisted(() => ({
  drop: undefined as undefined | ((event: { payload: { type: string; paths: string[] } }) => void),
  scan: vi.fn(),
  execute: vi.fn(),
  undo: vi.fn(),
  close: vi.fn(),
  show: vi.fn(),
  drag: vi.fn(),
  resize: vi.fn().mockResolvedValue(undefined),
  iconSize: vi.fn(async (value?: number) => value ?? 64),
  emit: vi.fn(),
  configChanged: undefined as undefined | (() => void),
  switchProfile: vi.fn().mockResolvedValue(undefined),
  popup: vi.fn(),
  menuItems: [] as { text: string; checked?: boolean; action?: () => void; items?: unknown[] }[],
}));
vi.mock("@tauri-apps/api/menu", () => {
  const item = {
    new: async (options: (typeof mocks.menuItems)[number]) => {
      mocks.menuItems.push(options);
      return options;
    },
  };
  return {
    MenuItem: item,
    CheckMenuItem: item,
    Submenu: item,
    PredefinedMenuItem: item,
    Menu: { new: async () => ({ popup: mocks.popup }) },
  };
});
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: async (callback: typeof mocks.drop) => {
      mocks.drop = callback;
      return () => {};
    },
    startDragging: mocks.drag,
    startResizeDragging: mocks.resize,
    setAlwaysOnTop: vi.fn().mockResolvedValue(undefined),
  }),
}));
vi.mock("@tauri-apps/api/event", () => ({
  emitTo: mocks.emit,
  listen: async (name: string, callback: () => void) => {
    if (name === "config-changed") mocks.configChanged = callback;
    return () => {};
  },
}));
vi.mock("../api", async (original) => ({
  ...(await original<typeof import("../api")>()),
  api: {
    getConfig: async () => ({ config: { profile: "工作", profiles: [{ name: "工作" }, { name: "个人" }] } }),
    switchProfile: mocks.switchProfile,
    scanPaths: mocks.scan,
    executePlan: mocks.execute,
    toggleFloat: mocks.close,
    showMain: mocks.show,
    floatResize: vi.fn().mockResolvedValue(undefined),
    floatIconSize: mocks.iconSize,
    undo: mocks.undo,
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

async function dragEvent(type: string, paths: string[] = []) {
  await act(async () => mocks.drop!({ payload: { type, paths } }));
}

beforeEach(() => {
  vi.clearAllMocks();
  localStorage.clear();
  mocks.menuItems = [];
  mocks.scan.mockImplementation(async (paths: string[]) => plan(paths));
  mocks.close.mockResolvedValue(false);
  mocks.show.mockResolvedValue(undefined);
  mocks.emit.mockResolvedValue(undefined);
  mocks.drag.mockResolvedValue(undefined);
  mocks.execute.mockResolvedValue({ batch_id: 1, done: 1, skipped: [], failed: [] });
  mocks.undo.mockResolvedValue(undefined);
});
afterEach(cleanup);

describe("悬浮窗文件列表", () => {
  it("滚轮切换接收类型并过滤旧条目，Ctrl 滚轮只调整大小", async () => {
    let time = 1000;
    const now = vi.spyOn(Date, "now").mockImplementation(() => time);
    try {
      mocks.scan.mockImplementation(async (paths: string[], mode?: string) => {
        const accepted = paths.filter((path) => !mode || (mode === "folder" ? path.endsWith(".folder") : !path.endsWith(".folder")));
        return { ...plan(accepted), accepted_paths: accepted };
      });
      const { container } = render(<FloatApp />);
      const wheel = (deltaY: number, ctrlKey = false) => {
        time += 300;
        const event = new WheelEvent("wheel", { deltaY, bubbles: true, cancelable: true });
        // happy-dom's WheelEvent omits MouseEvent modifier properties.
        Object.defineProperty(event, "ctrlKey", { value: ctrlKey });
        fireEvent(container.querySelector(".float-target")!, event);
        expect(event.defaultPrevented).toBe(true);
      };
      expect(container.querySelector(".mode-all")).not.toBeNull();
      wheel(100);
      await drop(["D:/files/a.pdf", "D:/files/project.folder"]);
      expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/a.pdf", "D:/files/project.folder"], "file");
      expect(screen.queryByText("project.folder")).toBeNull();
      wheel(100);
      await waitFor(() => expect(screen.queryByText("a.pdf")).toBeNull());
      await drop(["D:/files/b.pdf", "D:/files/project.folder"]);
      expect(screen.queryByText("b.pdf")).toBeNull();
      expect(screen.getByText("project.folder")).toBeTruthy();
      wheel(-100, true);
      expect(mocks.iconSize).toHaveBeenLastCalledWith(68);
      expect(container.querySelector(".mode-folder")).not.toBeNull();
      wheel(100);
      await waitFor(() => expect(container.querySelector(".mode-all")).not.toBeNull());
      expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/project.folder"]);
    } finally {
      now.mockRestore();
    }
  });
  it("右键方案菜单标记当前方案，支持切换与关闭悬浮窗", async () => {
    const { container } = render(<FloatApp />);
    await drop(["D:/files/a.pdf"]);
    fireEvent.contextMenu(container.querySelector(".float-target")!);
    await waitFor(() => expect(mocks.popup).toHaveBeenCalledTimes(1));
    expect(mocks.menuItems.find((item) => item.text === "工作")?.checked).toBe(true);
    mocks.menuItems.find((item) => item.text === "个人")?.action?.();
    expect(mocks.switchProfile).toHaveBeenCalledWith("个人");
    mocks.menuItems.find((item) => item.text === "关闭悬浮窗")?.action?.();
    await waitFor(() => expect(mocks.close).toHaveBeenCalledTimes(1));
  });

  it("整理完成并收起后切换方案，不会重新扫描已整理的文件", async () => {
    render(<FloatApp />);
    await drop(["D:/files/a.pdf"]);
    fireEvent.click(screen.getByRole("button", { name: "整理 1 个文件" }));
    await screen.findByText("已整理 1 项");
    fireEvent.click(screen.getByTitle("收起面板"));
    await act(async () => mocks.configChanged?.());
    expect(mocks.scan).toHaveBeenCalledTimes(1);
  });
  it("松开时路径为空，仍使用本次拖入的路径更新列表", async () => {
    render(<FloatApp />);
    await drop(["D:/files/old.pdf"]);
    await dragEvent("enter", ["D:/files/new.png"]);
    await dragEvent("drop");
    expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/old.pdf", "D:/files/new.png"]);
    expect(screen.getByText("new.png")).toBeTruthy();
  });

  it("取消拖拽不会添加文件，下一次无路径的松开也不会复用旧文件", async () => {
    render(<FloatApp />);
    await drop(["D:/files/old.pdf"]);
    await dragEvent("enter", ["D:/files/canceled.png"]);
    await dragEvent("leave");
    await dragEvent("drop");
    expect(mocks.scan).toHaveBeenCalledTimes(1);
    expect(screen.queryByText("canceled.png")).toBeNull();
    expect(screen.getByText("未能读取拖入的文件，请重新拖入或点击添加文件")).toBeTruthy();
    await dragEvent("enter", ["D:/files/next.png"]);
    await dragEvent("drop");
    expect(screen.getByText("next.png")).toBeTruthy();
    expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/old.pdf", "D:/files/next.png"]);
  });

  it("完成后收起再遇到无效拖入，不会重新启用上一批整理按钮", async () => {
    render(<FloatApp />);
    await drop(["D:/files/old.pdf"]);
    fireEvent.click(screen.getByRole("button", { name: "整理 1 个文件" }));
    await screen.findByText("已整理 1 项");
    fireEvent.click(screen.getByTitle("收起面板"));
    await dragEvent("drop");
    expect(screen.queryByRole("button", { name: "整理 1 个文件" })).toBeNull();
    expect(screen.getByText("已整理 1 项")).toBeTruthy();
    expect(mocks.execute).toHaveBeenCalledTimes(1);
  });

  it("整理期间拖入的新文件会排队，完成后刷新成下一批且不会自动执行", async () => {
    let complete!: (result: { batch_id: number; done: number; skipped: []; failed: [] }) => void;
    mocks.execute.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          complete = resolve;
        }),
    );
    render(<FloatApp />);
    await drop(["D:/files/old.pdf"]);
    fireEvent.click(screen.getByRole("button", { name: "整理 1 个文件" }));
    await drop(["D:/files/new.png"]);
    await drop(["D:/files/new.png", "D:/files/next.pdf"]);
    expect(mocks.scan).toHaveBeenCalledTimes(1);
    expect(screen.getByText("已接收 2 个新文件，当前整理完成后显示")).toBeTruthy();
    await act(async () => complete({ batch_id: 1, done: 1, skipped: [], failed: [] }));
    expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/new.png", "D:/files/next.pdf"]);
    expect(screen.getByText("new.png")).toBeTruthy();
    expect(screen.getByText("next.pdf")).toBeTruthy();
    expect(screen.queryByText("old.pdf")).toBeNull();
    expect(mocks.execute).toHaveBeenCalledTimes(1);
  });

  it("连续拖入时，较慢返回的旧扫描不会覆盖新列表", async () => {
    let oldScan!: (p: PlanView) => void;
    mocks.scan.mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          oldScan = resolve;
        }),
    );
    render(<FloatApp />);
    await drop(["D:/files/old.pdf"]);
    await drop(["D:/files/new.png"]);
    await act(async () => oldScan(plan(["D:/files/old.pdf"])));
    expect(screen.getByText("new.png")).toBeTruthy();
  });

  it("撤销期间拖入的文件会在撤销完成后显示，不被旧结果覆盖", async () => {
    let complete!: () => void;
    mocks.undo.mockImplementationOnce(
      () =>
        new Promise<void>((resolve) => {
          complete = resolve;
        }),
    );
    render(<FloatApp />);
    await drop(["D:/files/old.pdf"]);
    fireEvent.click(screen.getByRole("button", { name: "整理 1 个文件" }));
    fireEvent.click(await screen.findByRole("button", { name: "撤销" }));
    await drop(["D:/files/new.png"]);
    await act(async () => complete());
    expect(mocks.scan).toHaveBeenLastCalledWith(["D:/files/new.png"]);
    expect(screen.getByText("new.png")).toBeTruthy();
    expect(screen.queryByText("old.pdf")).toBeNull();
    expect(screen.queryByText("已撤销")).toBeNull();
  });
  it("展开窗口可以从边角调整大小，收起后没有调整手柄", async () => {
    const { container } = render(<FloatApp />);
    expect(container.querySelector(".float-resize")).toBeNull();
    await drop(["D:/files/a.pdf"]);
    fireEvent.mouseDown(screen.getByTitle("调整窗口大小"), { button: 0 });
    expect(mocks.resize).toHaveBeenCalledWith("SouthEast");
    expect(mocks.drag).not.toHaveBeenCalled();
    fireEvent.click(screen.getByTitle("收起面板"));
    expect(container.querySelector(".float-resize")).toBeNull();
  });

  it("关闭分组后连未匹配条目也恢复普通列表，再点分组图标可以切换", async () => {
    const { container } = render(<FloatApp />);
    await drop(["D:/files/a.pdf", "D:/files/b.unknown"]);
    fireEvent.click(screen.getByRole("button", { name: "按类型分组" }));
    expect(screen.getByRole("button", { name: "列表" }).getAttribute("aria-pressed")).toBe("true");
    expect(container.querySelector(".float-group-head")).toBeNull();
    expect(screen.getByText("b.unknown")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "按扩展名分组" }));
    expect(screen.getByRole("checkbox", { name: ".pdf" })).toBeTruthy();
  });
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
    fireEvent.click(screen.getByRole("button", { name: "按扩展名分组" }));
    fireEvent.click(screen.getByRole("checkbox", { name: ".pdf" }));
    fireEvent.click(screen.getByRole("button", { name: "列表" }));
    expect((screen.getByRole("checkbox", { name: "a.pdf" }) as HTMLInputElement).checked).toBe(false);
    expect(screen.getByRole("button", { name: "整理 1 个文件" })).toBeTruthy();
    expect(screen.queryByRole("button", { name: "折叠全部" })).toBeNull();
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

  it("右上角关闭按钮收起为悬浮框，不关闭窗口也不触发拖动", async () => {
    const { container } = render(<FloatApp />);
    await drop(["D:/files/a.pdf"]);
    const close = screen.getByTitle("收起面板");
    fireEvent.mouseDown(close, { button: 0, detail: 1 });
    fireEvent.click(close);
    expect(mocks.drag).not.toHaveBeenCalled();
    expect(mocks.close).not.toHaveBeenCalled();
    expect(container.querySelector(".float-root")).not.toBeNull();
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

  it("已在目标位置的文件仍显示，且不能重复整理", async () => {
    const p = plan([]);
    p.in_place = 1;
    p.skipped.push({
      path: "D:/files/done.pdf",
      name: "done.pdf",
      display: "D:/files/done.pdf",
      kind: "in_place",
      label: "已在目标位置",
      reason: "已在目标位置，无需整理",
      rule: "文档",
    });
    mocks.scan.mockResolvedValueOnce(p);
    render(<FloatApp />);
    await drop(["D:/files/done.pdf"]);
    expect(screen.getByText("done.pdf")).toBeTruthy();
    expect(screen.getByText("已在目标位置")).toBeTruthy();
    expect((screen.getByRole("button", { name: "没有可整理的文件" }) as HTMLButtonElement).disabled).toBe(true);
    expect(screen.queryByText("拖入文件或文件夹")).toBeNull();
    expect(mocks.execute).not.toHaveBeenCalled();
  });

  it("找不到拖入的文件时显示扫描警告", async () => {
    const p = plan([]);
    p.warnings.push("找不到 D:/files/gone.pdf");
    mocks.scan.mockResolvedValueOnce(p);
    render(<FloatApp />);
    await drop(["D:/files/gone.pdf"]);
    expect(screen.getByText("找不到 D:/files/gone.pdf")).toBeTruthy();
  });
});
