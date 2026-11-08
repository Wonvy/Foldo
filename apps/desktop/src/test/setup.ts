// 测试默认使用简体中文（原文），结果不随运行测试的电脑的系统语言变化
Object.defineProperty(globalThis, "navigator", {
  value: { language: "zh-CN", languages: ["zh-CN"], userAgent: globalThis.navigator?.userAgent ?? "node" },
  configurable: true,
});
