import { StrictMode } from "react";
import { createRoot } from "react-dom/client";
import { lang, loadLocale } from "./i18n";
import { loadTheme } from "./theme";
import "./styles.css";

// 先设好主题，避免首屏闪一下浅色
document.documentElement.dataset.theme = loadTheme();

// 悬浮窗和主窗口共用同一套前端，用地址里的 #float 区分
const isFloat = location.hash === "#float";
const isRegex = location.hash.startsWith("#regex/");
if (isFloat) document.documentElement.classList.add("float");

// 开发时在普通浏览器里打开（不在 Tauri 中）：注入模拟后端，方便预览界面。
// 正式构建时 import.meta.env.DEV 为 false，这段代码和模拟数据都不会打包进去。
const inTauri = "__TAURI_INTERNALS__" in window;
const ready = import.meta.env.DEV && !inTauri ? import("./dev/mock").then((m) => m.installMock()) : Promise.resolve();

// 先加载译文，再加载界面（界面模块里有些文字在加载时就翻译好）
void Promise.all([ready, loadLocale()]).then(async () => {
  const { api } = await import("./api");
  // 后台的提示、通知和托盘菜单使用同一种语言
  if (!isFloat && !isRegex) void api.setLocale(lang).catch(() => {});
  // 主窗口切换语言后，悬浮窗也重新加载
  if (isFloat || isRegex) window.addEventListener("storage", (e) => e.key === "foldo.lang" && location.reload());
  const root = createRoot(document.getElementById("root")!);
  if (isRegex) {
    const { default: RegexLabApp } = await import("./regex/RegexLabApp");
    root.render(
      <StrictMode>
        <RegexLabApp id={location.hash.slice(7)} />
      </StrictMode>,
    );
  } else if (isFloat) {
    const { default: FloatApp } = await import("./float/FloatApp");
    root.render(
      <StrictMode>
        <FloatApp />
      </StrictMode>,
    );
  } else {
    const { default: App } = await import("./App");
    root.render(
      <StrictMode>
        <App />
      </StrictMode>,
    );
  }
});
