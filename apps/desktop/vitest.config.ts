import { defineConfig } from "vitest/config";

// Pure logic runs in Node; interactive component tests opt into happy-dom.
export default defineConfig({
  test: {
    include: ["src/**/*.test.{ts,tsx}"],
    setupFiles: ["src/test/setup.ts"],
  },
  server: { fs: { allow: ["..", "../../locales"] } },
});
