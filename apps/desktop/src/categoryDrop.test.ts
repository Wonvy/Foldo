// @vitest-environment happy-dom
import { afterEach, describe, expect, it, vi } from "vitest";
import { PhysicalPosition } from "@tauri-apps/api/dpi";
import { CATEGORY_DROP, createCategoryDropRouter, type CategoryDrop } from "./categoryDrop";

afterEach(() => {
  document.body.innerHTML = "";
  vi.restoreAllMocks();
});
describe("分类拖入路由", () => {
  it("按 DPI 定位目标分类，恢复空 drop，取消和隐藏页面不会误添加", () => {
    document.body.innerHTML = '<div><section id="settings-panel-categories"><div data-category-drop="installer"></div></section></div>';
    const panel = document.getElementById("settings-panel-categories")!;
    vi.spyOn(window, "devicePixelRatio", "get").mockReturnValue(2);
    const hit = vi.spyOn(document, "elementFromPoint").mockReturnValue(panel.firstElementChild);
    const received: CategoryDrop[] = [];
    const listener = (e: Event) => received.push((e as CustomEvent<CategoryDrop>).detail);
    window.addEventListener(CATEGORY_DROP, listener);
    const route = createCategoryDropRouter();
    const position = new PhysicalPosition(200, 100);
    expect(route({ type: "enter", paths: ["C:/Shapr3D.Package.appinstaller"], position })).toBe(true);
    expect(hit).toHaveBeenCalledWith(100, 50);
    route({ type: "drop", paths: [], position });
    expect(received[received.length - 1]).toEqual({ type: "drop", category: "installer", paths: ["C:/Shapr3D.Package.appinstaller"] });
    route({ type: "enter", paths: ["C:/cancel.exe"], position });
    route({ type: "leave" });
    route({ type: "drop", paths: [], position });
    expect(received[received.length - 1]?.paths).toEqual([]);
    panel.parentElement!.hidden = true;
    expect(route({ type: "drop", paths: ["C:/hidden.exe"], position })).toBe(false);
    window.removeEventListener(CATEGORY_DROP, listener);
  });
});
