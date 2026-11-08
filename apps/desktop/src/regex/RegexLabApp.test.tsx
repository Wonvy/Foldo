// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import RegexLabApp from "./RegexLabApp";
const mocks = vi.hoisted(() => ({
  drop: undefined as undefined | ((e: { payload: { type: string; paths: string[] } }) => void),
  apply: vi.fn().mockResolvedValue(undefined),
  names: vi.fn().mockResolvedValue({ names: ["IMG_001.png", "IMG_002.png"], ignored: 0 }),
  test: vi.fn(),
}));
vi.mock("@tauri-apps/api/window", () => ({
  getCurrentWindow: () => ({
    onDragDropEvent: async (callback: typeof mocks.drop) => {
      mocks.drop = callback;
      return () => {};
    },
  }),
}));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn() }));
vi.mock("../api", async (original) => ({
  ...(await original<typeof import("../api")>()),
  api: {
    regexSession: async () => ({ id: "test", pattern: "", caseSensitive: false, canApply: true }),
    regexFileSamples: mocks.names,
    applyRegexLab: mocks.apply,
    testRegex: mocks.test,
  },
}));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
it("拖入多个样本自动生成，反例阻止误应用，应用保留原条件会话", async () => {
  mocks.test.mockImplementation(async (pattern: string, names: string) =>
    names
      ? names.split("\n").map((text) => {
          const m = new RegExp(pattern, "u").exec(text);
          return { text, range: m ? [m.index, m.index + m[0].length] : null, captures: [] };
        })
      : [],
  );
  render(<RegexLabApp id="test" />);
  await screen.findByText("用文件名举例");
  await waitFor(() => expect(mocks.drop).toBeTruthy());
  act(() => mocks.drop!({ payload: { type: "enter", paths: ["C:/IMG_001.png", "C:/IMG_002.png"] } }));
  act(() => mocks.drop!({ payload: { type: "drop", paths: [] } }));
  await screen.findByDisplayValue("IMG_002.png");
  await waitFor(() => expect((screen.getByText("应用到条件") as HTMLButtonElement).disabled).toBe(false));
  fireEvent.click(screen.getAllByRole("checkbox", { name: "应该匹配" })[1]);
  expect((screen.getByText("应用到条件") as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(screen.getAllByRole("checkbox", { name: "应该匹配" })[1]);
  fireEvent.click(screen.getByText("应用到条件"));
  await waitFor(() => expect(mocks.apply).toHaveBeenCalledWith("test", expect.stringContaining("[0-9]{3}"), false));
});
