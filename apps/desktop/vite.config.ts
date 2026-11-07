import { defineConfig } from "vite";
import react from "@vitejs/plugin-react";

// Tauri 开发模式固定使用 1420 端口（见 src-tauri/tauri.conf.json 的 devUrl）
export default defineConfig({
  plugins: [react()],
  clearScreen: false,
  server: {
    port: 1420,
    strictPort: true,
    watch: { ignored: ["**/src-tauri/**"] },
  },
  build: {
    // Windows 用 WebView2（Chromium），macOS 用 WKWebView（Safari）
    target: ["es2021", "chrome105", "safari15"],
  },
});
