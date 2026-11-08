import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import App from "./App";
import FloatApp from "./float/FloatApp";
import { lang } from "./i18n";
import { loadTheme } from "./theme";
import "./styles.css";

// 先设好主题，避免首屏闪一下浅色
document.documentElement.dataset.theme = loadTheme();

// 悬浮窗和主窗口共用同一套前端，用地址里的 #float 区分
const isFloat = location.hash === "#float";
if (isFloat) document.documentElement.classList.add("float");

// 开发时在普通浏览器里打开（不在 Tauri 中）：注入模拟后端，方便预览界面。
// 正式构建时 import.meta.env.DEV 为 false，这段代码和模拟数据都不会打包进去。
const inTauri = "__TAURI_INTERNALS__" in window;
const ready = import.meta.env.DEV && !inTauri ? import("./dev/mock").then((m) => m.installMock()) : Promise.resolve();

void ready.then(() => {
  // 后台的提示、通知和托盘菜单使用同一种语言
  if (!isFloat) void import("./api").then(({ api }) => api.setLocale(lang).catch(() => {}));
  // 主窗口切换语言后，悬浮窗也重新加载
  if (isFloat) window.addEventListener("storage", (e) => e.key === "foldo.lang" && location.reload());
  createRoot(document.getElementById("root")!).render(<StrictMode>{isFloat ? <FloatApp /> : <App />}</StrictMode>);
});
