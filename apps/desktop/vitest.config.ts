import { defineConfig } from "vitest/config";

// 前端单元测试：只测纯逻辑（规则与配置的互相转换、描述文字、翻译），在 Node 里运行
export default defineConfig({
  test: {
    include: ["src/**/*.test.ts"],
    setupFiles: ["src/test/setup.ts"],
  },
  server: { fs: { allow: ["..", "../../locales"] } },
});
