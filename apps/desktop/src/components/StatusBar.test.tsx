// @vitest-environment happy-dom
import { act, cleanup, render, screen } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import StatusBar from "./StatusBar";
import { withSaveStatus } from "../saveStatus";
vi.mock("../store", () => ({ useStore: () => ({ config: { profile: "默认方案" } }) }));
afterEach(cleanup);
it("保存进行中、失败和重试成功均显示反馈，并发成功不会覆盖失败", async () => {
  render(<StatusBar />);
  let finish!: () => void;
  let pending!: Promise<void>;
  act(() => {
    pending = withSaveStatus(
      () =>
        new Promise<void>((resolve) => {
          finish = resolve;
        }),
    );
  });
  expect(screen.getByText("正在保存…")).toBeTruthy();
  await act(async () => {
    await withSaveStatus(async () => {
      throw new Error("磁盘已满");
    }).catch(() => {});
  });
  await act(async () => {
    finish();
    await pending;
  });
  expect(screen.getByText("保存失败：磁盘已满")).toBeTruthy();
  await act(async () => {
    await withSaveStatus(async () => {});
  });
  expect(screen.getByText("已保存")).toBeTruthy();
});
