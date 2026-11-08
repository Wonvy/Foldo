// 在真实的 Foldo 窗口里跑一遍主流程：看到整理建议 → 执行 → 撤销 → 新建规则并用在文件夹上 → 切换语言。
//
// 经由 tauri-driver（WebDriver）驱动应用，只能在 Windows 和 Linux 上运行（macOS 的 WebView 没有 WebDriver）。
// 每次运行都在临时目录里准备沙盒文件夹和配置，不会碰到真实文件。
//
// 环境变量：
//   FOLDO_APP      应用程序路径，默认 target/debug/foldo-desktop(.exe)
//   TAURI_DRIVER   tauri-driver 路径，默认从 PATH 里找
//   NATIVE_DRIVER  Windows 上 msedgedriver.exe 的路径
import assert from "node:assert/strict";
import { execFile, spawn } from "node:child_process";
import fs from "node:fs";
import net from "node:net";
import os from "node:os";
import path from "node:path";
import { after, before, test } from "node:test";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { Builder, By, Capabilities, until } from "selenium-webdriver";

const here = path.dirname(fileURLToPath(import.meta.url));
const repo = path.resolve(here, "../../..");
const exe = process.platform === "win32" ? ".exe" : "";
const app = process.env.FOLDO_APP ?? path.join(repo, "target/debug", `foldo-desktop${exe}`);
const artifacts = path.join(here, "artifacts");
const execFileAsync = promisify(execFile);

// ---------- 沙盒 ----------

const sandbox = fs.mkdtempSync(path.join(os.tmpdir(), "foldo-e2e-"));
const inbox = path.join(sandbox, "inbox");
const data = path.join(sandbox, "data");
const rulesFile = path.join(sandbox, "rules.yaml");

function file(rel, body) {
  const p = path.join(inbox, rel);
  fs.mkdirSync(path.dirname(p), { recursive: true });
  fs.writeFileSync(p, body);
  // 修改时间设到很久以前，不受“新文件缓冲期”影响
  const old = new Date("2024-05-01T12:00:00Z");
  fs.utimesSync(p, old, old);
  return p;
}

file("报告.pdf", "pdf");
file("照片.jpg", "jpg");
fs.mkdirSync(data, { recursive: true });
// YAML 单引号字符串里的路径（Windows 的反斜杠不需要转义）
const q = (p) => `'${p.replace(/'/g, "''")}'`;
fs.writeFileSync(
  rulesFile,
  `version: 2
folders:
  - path: ${q(inbox)}
    rules: [文档归档]
rules:
  - name: 文档归档
    when: { ext: pdf }
    actions: [{ move: '文档/' }]
`,
);

// ---------- 启动 tauri-driver 和应用 ----------

let driverProc;
let driverLog = "";
/** @type {import("selenium-webdriver").WebDriver} */
let driver;

function waitPort(port, timeout = 30_000) {
  const end = Date.now() + timeout;
  return new Promise((resolve, reject) => {
    const tryOnce = () => {
      const s = net.connect(port, "127.0.0.1", () => {
        s.end();
        resolve();
      });
      s.on("error", () => (Date.now() > end ? reject(new Error(`端口 ${port} 没有打开`)) : setTimeout(tryOnce, 300)));
    };
    tryOnce();
  });
}

/** 把失败原因写成 GitHub 注解（不登录也能在 API 里看到），再继续抛出。 */
function report(name, err) {
  const msg = String(err?.stack ?? err)
    .split("\n")
    .slice(0, 6)
    .join(" | ");
  console.log(`::error title=E2E ${name}::${msg}`);
  if (driverLog) console.log(`::error title=E2E driver log::${driverLog.slice(-1500).replace(/\r?\n/g, " | ")}`);
}

async function screenshot(name) {
  try {
    fs.mkdirSync(artifacts, { recursive: true });
    fs.writeFileSync(path.join(artifacts, `${name}.png`), await driver.takeScreenshot(), "base64");
  } catch {
    /* 截图失败不影响结果 */
  }
}

/** 一个测试步骤：失败时截图并写注解。 */
function step(name, fn) {
  test(name, async () => {
    try {
      await fn();
    } catch (err) {
      await screenshot(name.replace(/[^\w一-鿿-]+/g, "_"));
      report(name, err);
      throw err;
    }
  });
}

before(async () => {
  try {
    assert.ok(fs.existsSync(app), `找不到应用：${app}（先运行 npx tauri build --debug --no-bundle）`);
    const args = process.env.NATIVE_DRIVER ? ["--native-driver", process.env.NATIVE_DRIVER] : [];
    driverProc = spawn(process.env.TAURI_DRIVER ?? "tauri-driver", args, {
      // 应用从 tauri-driver 继承环境变量：使用沙盒里的配置和数据目录，界面固定为简体中文
      env: { ...process.env, FOLDO_RULES: rulesFile, FOLDO_HOME: data },
      stdio: ["ignore", "pipe", "pipe"],
    });
    driverProc.stdout.on("data", (d) => (driverLog += d));
    driverProc.stderr.on("data", (d) => (driverLog += d));
    await waitPort(4444);

    const caps = new Capabilities();
    caps.set("tauri:options", { application: app });
    caps.setBrowserName("wry");
    driver = await new Builder().withCapabilities(caps).usingServer("http://127.0.0.1:4444/").build();
    await driver.manage().setTimeouts({ implicit: 0 });
  } catch (err) {
    report("启动", err);
    throw err;
  }
});

after(async () => {
  try {
    await driver?.quit();
  } catch {
    /* 已经退出 */
  }
  driverProc?.kill();
});

// ---------- 小工具 ----------

const byTest = (id) => By.css(`[data-testid="${id}"]`);
/** 页面上（可见的）包含这段文字的元素。 */
const byText = (text) => By.xpath(`//*[contains(normalize-space(text()), ${JSON.stringify(text)})]`);

async function find(locator, timeout = 20_000) {
  const el = await driver.wait(until.elementLocated(locator), timeout);
  await driver.wait(until.elementIsVisible(el), timeout);
  return el;
}

async function click(locator, timeout = 20_000) {
  const el = await find(locator, timeout);
  await driver.wait(until.elementIsEnabled(el), timeout);
  await el.click();
}

/** 等到条件成立（检查磁盘上的文件之类）。 */
async function eventually(check, message, timeout = 20_000) {
  const end = Date.now() + timeout;
  while (Date.now() < end) {
    if (await check()) return;
    await new Promise((r) => setTimeout(r, 250));
  }
  assert.fail(message);
}

async function setInput(locator, value) {
  const el = await find(locator);
  await el.clear();
  await el.sendKeys(value);
}

// ---------- 主流程 ----------

step("整理页列出按规则整理的文件", async () => {
  // 打开就会自动扫描：报告.pdf 有整理建议；照片.jpg 没有匹配的规则（按扩展名分组，默认折叠）
  await find(byText("报告.pdf"), 40_000);
  await find(By.css("#no-match"));
  await find(byTest("execute"));
});

step("重复打开只保留已有实例", async () => {
  const window = await driver.getWindowHandle();
  if (process.platform === "linux") {
    const { stdout } = await execFileAsync("dbus-send", [
      "--session", "--print-reply", "--dest=org.freedesktop.DBus", "/org/freedesktop/DBus",
      "org.freedesktop.DBus.NameHasOwner", "string:com.wonvy.foldo.SingleInstance",
    ], { timeout: 5_000 });
    assert.match(stdout, /boolean true/, "原实例未注册到测试会话总线");
  }
  // 同时重复手动启动和静默自启；第二批进程必须正常退出，原窗口继续可用。
  await Promise.all(
    [[], [], ["--hidden"]].map((args) =>
      execFileAsync(app, args, {
        env: { ...process.env, FOLDO_RULES: rulesFile, FOLDO_HOME: data },
        timeout: 15_000,
        windowsHide: true,
      }),
    ),
  );
  assert.equal(await driver.getWindowHandle(), window);
  await find(byTest("execute"));
});

step("执行后文件真的被移动", async () => {
  await click(byTest("execute"));
  await find(byTest("result"));
  await eventually(() => fs.existsSync(path.join(inbox, "文档", "报告.pdf")), "报告.pdf 没有移到 文档/");
  assert.ok(!fs.existsSync(path.join(inbox, "报告.pdf")));
});

step("撤销后文件回到原处", async () => {
  await click(byTest("undo-result"));
  await click(byTest("undo-confirm"));
  await eventually(() => fs.existsSync(path.join(inbox, "报告.pdf")), "撤销后 报告.pdf 没有回到原处");
  assert.ok(!fs.existsSync(path.join(inbox, "文档", "报告.pdf")));
});

step("新建规则并加到文件夹后生效", async () => {
  await click(byTest("nav-rules"));
  await click(byTest("new-rule"));
  // 新规则的默认条件是“类型 是 图片”
  await setInput(By.css(".modal .title-input"), "图片归档");
  await setInput(By.css(".modal .dest-input"), "图片/");
  await click(byTest("rule-save"));
  // 只返回布尔值，避免弹窗正在卸载时 WebDriver 返回已失效的元素引用。
  await driver.wait(async () => await driver.executeScript("return document.querySelector('.modal') === null"), 10_000);

  // 在整理页把规则加到文件夹（添加后会自动重新扫描）
  await click(byTest("nav-tidy"));
  await click(By.css(".folder-row .add-btn"));
  await click(By.xpath(`//div[contains(@class,"menu")]//button[contains(., "图片归档")]`));
  await find(By.css(`.folder-row [data-rule-name="图片归档"]`));

  // 两条规则都生效：执行后 pdf 进 文档/，jpg 进 图片/
  await driver.sleep(500);
  await click(byTest("execute"));
  await find(byTest("result"));
  await eventually(() => fs.existsSync(path.join(inbox, "图片", "照片.jpg")), "照片.jpg 没有移到 图片/");
  await eventually(() => fs.existsSync(path.join(inbox, "文档", "报告.pdf")), "报告.pdf 没有移到 文档/");

  // 规则写进了配置文件，且加到了文件夹
  const yaml = fs.readFileSync(rulesFile, "utf8");
  assert.match(yaml, /图片归档/);
  assert.match(yaml, /rules:\s*\n?\s*- 文档归档\s*\n\s*- 图片归档|rules: \[文档归档, 图片归档\]/);
});

step("正则助手独立窗口按样本生成并解释表达式", async () => {
  await click(byTest("nav-rules"));
  const mainWindow = await driver.getWindowHandle();
  const before = new Set(await driver.getAllWindowHandles());
  await click(By.css('button[title="正则助手"]'));
  const lab = await driver.wait(async () => (await driver.getAllWindowHandles()).find((handle) => !before.has(handle)), 15_000);
  await driver.switchTo().window(lab);
  await find(By.css(".regex-lab"));
  await click(By.xpath('//button[contains(., "手动添加名称")]'));
  await setInput(By.css('input[aria-label="示例名称 1"]'), "IMG_001.png");
  await click(By.xpath('//button[contains(., "手动添加名称")]'));
  await setInput(By.css('input[aria-label="示例名称 2"]'), "IMG_002.png");
  await click(By.xpath('//button[contains(., "根据样本生成")]'));
  await driver.wait(async () => (await (await find(By.css(".regex-field textarea"))).getAttribute("value")) === "^IMG_[0-9]{3}\\.png$", 10_000);
  await find(By.css('.regex-token[title="前一项恰好重复 3 次"]'));
  await driver.wait(async () => (await driver.findElements(By.css(".regex-sample-row.ok"))).length === 2, 10_000);
  await driver.close();
  await driver.switchTo().window(mainWindow);
  await find(byTest("new-rule"));
});

step("切换语言后界面和后台都换过来", async () => {
  await click(byTest("nav-settings"));
  const select = await find(By.xpath(`//select[option[@value="ja"]]`));
  await select.findElement(By.css('option[value="ja"]')).click();
  // 切换后窗口重新加载
  await driver.wait(async () => (await driver.executeScript("return document.documentElement.lang")) === "ja", 20_000);
  const title = await (await find(byTest("nav-rules"))).getAttribute("title");
  assert.equal(title, "ルール");
  // 后台记住了语言（托盘和通知用）
  await eventually(() => fs.existsSync(path.join(data, "locale")) && fs.readFileSync(path.join(data, "locale"), "utf8").trim() === "ja", "后台没有切换到日语");
});
