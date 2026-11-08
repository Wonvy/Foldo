// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import RegexTester from "./RegexTester";
import type { RegexSession } from "../api";
const mocks = vi.hoisted(() => ({
  open: vi.fn().mockResolvedValue(undefined),
  close: vi.fn().mockResolvedValue(undefined),
  callbacks: new Map<string, (event: { payload: RegexSession }) => void>(),
}));
vi.mock("../api", async (original) => ({
  ...(await original<typeof import("../api")>()),
  api: { openRegexLab: mocks.open, closeRegexLab: mocks.close },
}));
vi.mock("@tauri-apps/api/event", () => ({
  listen: async (name: string, callback: (event: { payload: RegexSession }) => void) => {
    mocks.callbacks.set(name, callback);
    return () => mocks.callbacks.delete(name);
  },
}));
afterEach(() => {
  cleanup();
  mocks.callbacks.clear();
  vi.clearAllMocks();
});
it("子窗口带入当前表达式，只向对应条件回填，编辑器关闭后清理窗口", async () => {
  const first = vi.fn();
  const second = vi.fn();
  const ui = render(
    <>
      <RegexTester pattern="^first" onApply={first} />
      <RegexTester pattern="^second" onApply={second} />
    </>,
  );
  fireEvent.click(screen.getAllByTitle("正则助手")[1]);
  await waitFor(() => expect(mocks.open).toHaveBeenCalled());
  const session = mocks.open.mock.calls[0][0] as RegexSession;
  expect(session.pattern).toBe("^second");
  expect(session.canApply).toBe(true);
  act(() => mocks.callbacks.get(`regex-result-${session.id}`)!({ payload: { ...session, pattern: "^updated", caseSensitive: true } }));
  expect(second).toHaveBeenCalledWith("^updated", true);
  expect(first).not.toHaveBeenCalled();
  const callback = mocks.callbacks.get(`regex-result-${session.id}`)!;
  ui.unmount();
  act(() => callback({ payload: { ...session, pattern: "stale" } }));
  expect(second).toHaveBeenCalledTimes(1);
  expect(mocks.close).toHaveBeenCalledWith(session.id);
});
