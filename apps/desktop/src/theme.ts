// 主题：跟随系统 / 浅色 / 深色。选择保存在本机（localStorage），读写失败时退回“跟随系统”。
import { getCurrentWindow } from "@tauri-apps/api/window";

export type ThemePref = "system" | "light" | "dark";

const KEY = "foldo.theme";

export function loadTheme(): ThemePref {
  try {
    const v = localStorage.getItem(KEY);
    if (v === "light" || v === "dark" || v === "system") return v;
  } catch {
    /* 存储不可用 */
  }
  return "system";
}

export function applyTheme(pref: ThemePref) {
  document.documentElement.dataset.theme = pref;
  try {
    localStorage.setItem(KEY, pref);
  } catch {
    /* 存储不可用 */
  }
  // 同步原生窗口（标题栏）的明暗；不在 Tauri 中运行时忽略
  try {
    void getCurrentWindow()
      .setTheme(pref === "system" ? null : pref)
      .catch(() => {});
  } catch {
    /* 浏览器预览 */
  }
}
