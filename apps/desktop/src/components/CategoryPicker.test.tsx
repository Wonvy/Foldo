// @vitest-environment happy-dom
import { cleanup, fireEvent, render, screen, within } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import CategoryPicker from "./CategoryPicker";

const store = vi.hoisted(() => ({
  categories: [
    { id: "image", name: "图片", aliases: [], extensions: ["jpg", "png", "raw"] },
    { id: "installer", name: "安装包", aliases: [], extensions: ["exe", "msi", "msix", "deb", "rpm"] },
    { id: "font", name: "字体", aliases: [], extensions: ["ttf"] },
  ],
  config: { settings: { category_extensions: { image: ["png", "custom"], font: [] } } },
}));
vi.mock("../store", () => ({ useStore: () => store }));
afterEach(cleanup);
const options = ["图片", "安装包", "字体"];

describe("文件类型的后缀提示", () => {
  it("悬停显示完整后缀和自定义覆盖，空分类不会回退到默认后缀，悬停不改变选值", () => {
    const change = vi.fn();
    render(<CategoryPicker options={options} value="图片" onChange={change} />);
    fireEvent.click(screen.getByRole("button", { name: "文件类型" }));
    let detail = within(screen.getByRole("tooltip"));
    expect(detail.getByText(".custom")).toBeTruthy();
    expect(detail.queryByText(".jpg")).toBeNull();
    fireEvent.mouseEnter(screen.getByRole("option", { name: "安装包" }));
    detail = within(screen.getByRole("tooltip"));
    for (const ext of store.categories[1].extensions) expect(detail.getByText(`.${ext}`)).toBeTruthy();
    fireEvent.mouseEnter(screen.getByRole("option", { name: "字体" }));
    expect(detail.getByText("未配置扩展名")).toBeTruthy();
    expect(detail.queryByText(".ttf")).toBeNull();
    expect(change).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("option", { name: "安装包" }));
    expect(change).toHaveBeenCalledWith("安装包");
    expect(screen.queryByRole("listbox")).toBeNull();
  });

  it("键盘方向键显示提示，Esc 取消保留原值，Enter 选择并还原焦点", () => {
    const change = vi.fn();
    render(<CategoryPicker options={options} value="图片" onChange={change} />);
    const trigger = screen.getByRole("button", { name: "文件类型" });
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "ArrowDown" });
    expect(within(screen.getByRole("tooltip")).getByText(".exe")).toBeTruthy();
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "Escape" });
    expect(change).not.toHaveBeenCalled();
    expect(screen.queryByRole("listbox")).toBeNull();
    expect(document.activeElement).toBe(trigger);
    fireEvent.keyDown(trigger, { key: "ArrowDown" });
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "End" });
    fireEvent.keyDown(screen.getByRole("listbox"), { key: "Enter" });
    expect(change).toHaveBeenCalledWith("字体");
    expect(document.activeElement).toBe(trigger);
  });

  it("多选条件保留选择行为，并给类型按钮提供完整后缀提示", () => {
    const change = vi.fn();
    render(<CategoryPicker options={options} value={["图片"]} multiple onChange={change} />);
    const installer = screen.getByRole("button", { name: "安装包" });
    expect(installer.title).toBe(".exe  .msi  .msix  .deb  .rpm");
    fireEvent.click(installer);
    expect(change).toHaveBeenCalledWith(["图片", "安装包"]);
  });
});
