// @vitest-environment happy-dom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import CategorySettings from "./CategorySettings";
import { CATEGORY_DROP } from "../categoryDrop";
const mocks = vi.hoisted(() => ({ save: vi.fn().mockResolvedValue(undefined), extensions: vi.fn() }));
vi.mock("../store", () => ({
  useStore: () => ({
    config: { version: 2, settings: {}, rules: [] },
    categories: [
      { id: "installer", name: "安装包", extensions: ["exe"] },
      { id: "image", name: "图片", extensions: ["jpg"] },
    ],
    save: mocks.save,
  }),
}));
vi.mock("../api", async (original) => ({ ...(await original<typeof import("../api")>()), api: { droppedExtensions: mocks.extensions } }));
afterEach(() => {
  cleanup();
  vi.clearAllMocks();
});
const drop = (category: string, paths: string[]) =>
  act(() => {
    window.dispatchEvent(new CustomEvent(CATEGORY_DROP, { detail: { type: "drop", category, paths } }));
  });

describe("拖入文件添加分类扩展名", () => {
  it("只添加识别出的后缀，保存前有明确提示，已有分类冲突不会覆盖", async () => {
    mocks.extensions.mockResolvedValue({ extensions: ["appinstaller"], ignored: 1 });
    render(<CategorySettings />);
    drop("installer", ["C:/Shapr3D.Package.appinstaller", "C:/folder"]);
    await screen.findByText(".appinstaller");
    expect(mocks.save).not.toHaveBeenCalled();
    expect(screen.getByText("有未保存的修改")).toBeTruthy();
    fireEvent.click(screen.getByText("保存更改"));
    await waitFor(() =>
      expect(mocks.save).toHaveBeenCalledWith(
        expect.objectContaining({ settings: { category_extensions: { installer: ["exe", "appinstaller"] } } }),
      ),
    );
    mocks.extensions.mockResolvedValue({ extensions: ["jpg"], ignored: 0 });
    drop("installer", ["C:/photo.jpg"]);
    await screen.findByRole("alert");
    expect(screen.getByRole("alert").textContent).toContain("已属于“图片”");
    expect(mocks.save).toHaveBeenCalledTimes(1);
  });
});
