// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import FolderConditionInput from "./FolderConditionInput";
vi.mock("../store", () => ({
  pickFolder: async () => "C:/Downloads",
  useStore: () => ({
    fields: [],
    vars: [
      { key: "dir.desktop", label: "桌面", group: "系统文件夹", desc: "" },
      { key: "system.user", label: "用户名", group: "系统", desc: "" },
    ],
  }),
}));
afterEach(cleanup);
it("选择完整目录或位置变量，不混入其他变量，并可取消菜单", async () => {
  const change = vi.fn();
  render(<FolderConditionInput value="" onChange={change} />);
  fireEvent.click(screen.getByTitle("选择文件夹"));
  await waitFor(() => expect(change).toHaveBeenCalledWith("C:/Downloads"));
  fireEvent.click(screen.getByLabelText("插入变量"));
  expect(screen.queryByText("用户名")).toBeNull();
  fireEvent.click(screen.getByText("桌面"));
  expect(change).toHaveBeenLastCalledWith("{dir.desktop}");
});
