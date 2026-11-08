import type { DragDropEvent } from "@tauri-apps/api/webview";

export const CATEGORY_DROP = "foldo-category-drop";
export type CategoryDrop = { type: DragDropEvent["type"]; category: string | null; paths: string[] };

/** Route native drops before the main window's normal organize-files handler. */
export function createCategoryDropRouter() {
  let entered: string[] = [];
  return (event: DragDropEvent): boolean => {
    const panel = document.getElementById("settings-panel-categories");
    if (!panel || panel.closest("[hidden]")) {
      entered = [];
      return false;
    }
    if (event.type === "enter") entered = event.paths;
    const paths = event.type === "drop" ? (event.paths.length ? event.paths : entered) : [];
    let category: string | null = null;
    if (event.type !== "leave") {
      const scale = window.devicePixelRatio || 1;
      const row = document
        .elementFromPoint(event.position.x / scale, event.position.y / scale)
        ?.closest<HTMLElement>("[data-category-drop]");
      if (row && panel.contains(row)) category = row.dataset.categoryDrop ?? null;
    }
    window.dispatchEvent(new CustomEvent<CategoryDrop>(CATEGORY_DROP, { detail: { type: event.type, category, paths } }));
    if (event.type === "leave" || event.type === "drop") entered = [];
    return true;
  };
}
