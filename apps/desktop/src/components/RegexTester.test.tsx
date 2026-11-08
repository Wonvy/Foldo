// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import RegexTester from "./RegexTester";
import type { RegexSample } from "../api";

const testRegex = vi.hoisted(() => vi.fn());
vi.mock("../api", async (original) => ({ ...(await original<typeof import("../api")>()), api: { testRegex } }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});

describe("正则调试", () => {
  it("过期的测试结果不会覆盖新表达式，错误不能应用到规则", async () => {
    let resolveOld!: (value: RegexSample[]) => void;
    testRegex
      .mockImplementationOnce(
        () =>
          new Promise((resolve) => {
            resolveOld = resolve;
          }),
      )
      .mockRejectedValue("invalid regex");
    const apply = vi.fn();
    render(<RegexTester pattern="old" caseSensitive={false} onApply={apply} onClose={() => {}} />);
    await waitFor(() => expect(testRegex).toHaveBeenCalledTimes(1));
    fireEvent.change(screen.getByLabelText("正则表达式"), { target: { value: "[" } });
    await screen.findByText("invalid regex");
    await act(async () => resolveOld([{ text: "old", range: [0, 3], captures: [] }]));
    expect(screen.getByText("invalid regex")).toBeTruthy();
    expect((screen.getByText("应用到条件") as HTMLButtonElement).disabled).toBe(true);
    expect(apply).not.toHaveBeenCalled();
  });

  it("日期示例带入样本，并把正则和大小写选项一起应用到条件", async () => {
    testRegex.mockResolvedValue([]);
    const apply = vi.fn();
    const close = vi.fn();
    render(<RegexTester pattern="" caseSensitive={false} onApply={apply} onClose={close} />);
    fireEvent.click(screen.getByText("日期开头示例"));
    fireEvent.click(screen.getByRole("checkbox", { name: "区分大小写" }));
    await waitFor(() => expect((screen.getByText("应用到条件") as HTMLButtonElement).disabled).toBe(false));
    expect(testRegex.mock.lastCall?.[1]).toContain("26.11.8 xxx");
    fireEvent.click(screen.getByText("应用到条件"));
    expect(apply).toHaveBeenCalledWith(expect.stringContaining("^(?:"), true);
    expect(close).toHaveBeenCalledTimes(1);
  });
});
