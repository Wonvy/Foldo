# Foldo

[![Build](https://github.com/Wonvy/Foldo/actions/workflows/build.yml/badge.svg)](https://github.com/Wonvy/Foldo/actions/workflows/build.yml)

跨平台（macOS / Windows / Linux）的文件夹整理工具：用 YAML 写规则，先预览、再执行，执行过的操作可以撤销。

提供图形界面（Tauri）和命令行两种用法，两者共用同一份规则文件和操作日志。

当前进度：**M1 核心引擎 + CLI** 已完成；**M2 图形界面**第一阶段已完成（规则编辑与校验、预览勾选、执行、历史与撤销）。可视化规则编辑器、实时监听和定时运行在后续里程碑中实现。

## 项目结构

```
crates/
  foldo-core/   核心引擎，不依赖任何 UI
    config.rs     规则文件的 YAML 结构
    rule.rs       规则校验与编译
    condition.rs  条件匹配
    template.rs   路径模板 {created:%Y}/{stem|slug}
    scan.rs       目录扫描（跳过隐藏文件、下载中的文件、系统杂项文件）
    plan.rs       生成计划（只读，处理目标冲突）
    exec.rs       执行计划
    journal.rs    操作日志（SQLite）
    undo.rs       撤销
    fsops.rs      跨磁盘移动、带校验的复制、回收站
    paths.rs      ~ 展开、大小写不敏感比较、系统目录保护
  foldo-cli/    命令行，二进制名 foldo
apps/desktop/   图形界面（Tauri 2 + React + TypeScript）
  src/          前端页面：规则、预览与执行、历史
  src-tauri/    Rust 端，把 foldo-core 包装成前端可调用的命令
examples/rules.yaml  示例规则
```

## 下载

| 来源 | 内容 | 说明 |
|---|---|---|
| [Releases](https://github.com/Wonvy/Foldo/releases) | 正式版本 | 无需登录即可下载 |
| [Actions](https://github.com/Wonvy/Foldo/actions/workflows/build.yml) | main 分支每次提交的构建 | 打开某次运行，在页面底部的 Artifacts 里下载；需要登录 GitHub，保留 30 天 |

**图形界面安装包**

| 文件 | 平台 |
|---|---|
| `foldo-desktop-<版本>-windows-x64-setup.exe` | Windows 10 / 11（64 位），双击安装 |
| `foldo-desktop-<版本>-macos-arm64.dmg` | Apple 芯片 Mac |
| `foldo-desktop-<版本>-linux-x64.deb` | Debian / Ubuntu |

**命令行版**

| 文件 | 平台 |
|---|---|
| `foldo-<版本>-windows-x64.zip` | Windows 10 / 11（64 位） |
| `foldo-<版本>-macos-arm64.tar.gz` | Apple 芯片 Mac |
| `foldo-<版本>-macos-x64.tar.gz` | Intel Mac |
| `foldo-<版本>-linux-x64.tar.gz` | Linux（x86_64） |

每个压缩包里都有 `foldo` 程序、本说明和示例规则 `rules.example.yaml`。

### 在 Windows 上试用命令行版

1. 解压 zip，在解压出来的文件夹的地址栏输入 `powershell` 并回车，就会在这个文件夹里打开 PowerShell。
2. 新建一个测试目录，放几个文件进去，再生成一份只针对这个目录的规则（示例规则 `rules.example.yaml` 指向真实的“下载”和“桌面”文件夹，不适合拿来试）：

   ```powershell
   mkdir $HOME\FoldoTest\Inbox
   "a" > $HOME\FoldoTest\Inbox\报告.pdf
   "b" > $HOME\FoldoTest\Inbox\照片.jpg
   "c" > $HOME\FoldoTest\Inbox\笔记.txt

   @'
   version: 1
   rules:
     - name: 图片归档
       locations: ["~/FoldoTest/Inbox"]
       conditions:
         - extension: [jpg, png]
       actions:
         - move: "~/FoldoTest/图片/{modified:%Y}/{modified:%m}/"
     - name: 文档归档
       locations: ["~/FoldoTest/Inbox"]
       conditions:
         - extension: [pdf, docx]
       actions:
         - move: "文档/"
   '@ | Set-Content -Encoding utf8 test-rules.yaml
   ```

3. 按顺序运行：

   ```powershell
   .\foldo.exe --rules .\test-rules.yaml check
   .\foldo.exe --rules .\test-rules.yaml plan
   .\foldo.exe --rules .\test-rules.yaml run
   .\foldo.exe history
   .\foldo.exe undo
   ```

   在资源管理器里打开 `FoldoTest`，可以看到执行后和撤销后的变化。

程序还没有数字签名，Windows 可能会弹出“Windows 已保护你的电脑”。点击“更多信息”→“仍要运行”即可。

### 在 Windows 上试用图形界面

双击 `…-windows-x64-setup.exe` 安装，开始菜单里会出现 Foldo。安装程序和程序本身都还没有数字签名，同样需要点击“更多信息”→“仍要运行”。Windows 10 / 11 已自带 WebView2；如果缺失，安装程序会自动下载。

第一次打开时点击“生成示例规则”，**先把规则里的位置改成测试文件夹**（例如 `~/FoldoTest/Inbox`），再到“预览与执行”里试。

### 发布新版本

修改 `Cargo.toml` 里的 `version`，提交后打标签并推送，GitHub 会自动构建并创建 Release：

```bash
git tag v0.1.0
git push origin v0.1.0
```

带连字符的标签（如 `v0.2.0-beta.1`）会被标记为预发布版本。

## 从源码构建

```bash
cargo build --release
./target/release/foldo init        # 生成示例规则
./target/release/foldo check       # 校验规则
./target/release/foldo plan        # 预览，不改动任何文件
./target/release/foldo run         # 预览并确认后执行（-y 跳过确认）
./target/release/foldo history     # 执行历史
./target/release/foldo undo [批次]  # 撤销，默认最近一次
```

规则文件默认位于系统配置目录下的 `foldo/rules.yaml`（macOS：`~/Library/Application Support/foldo/rules.yaml`），可以用 `--rules` 或环境变量 `FOLDO_RULES` 指定。操作日志默认放在系统数据目录下的 `foldo/`，可以用 `--data-dir` 或 `FOLDO_HOME` 指定。

## 规则参考

```yaml
version: 1
rules:
  - name: 规则名（唯一）
    enabled: true            # 默认 true
    locations: ["~/Downloads"]
    recursive: false         # 是否扫描子文件夹
    include_hidden: false    # 是否处理隐藏文件
    match: all               # all（默认）| any
    conditions: [...]        # 为空表示匹配所有文件
    actions: [...]
```

**执行语义**

- 规则从上到下依次生效，每个文件只由第一条命中它的规则处理。
- 一条规则里的多个动作依次作用于文件当前的位置，比如先 `rename` 再 `move`。
- 某个文件的动作链中只要有一步无法完成，这个文件就整体跳过，不会只做一半。

**条件**

| 条件 | 示例 |
|---|---|
| `extension` | `extension: [jpg, png]`（不区分大小写） |
| `name` | `name: "*.pdf"`，或 `name: { regex: '^IMG_(\d+)', contains: x, starts_with: x, ends_with: x, case_sensitive: false }` |
| `size` | `size: { gt: 50KB, lte: 1GiB }`（KB 按 1000 进制，KiB 按 1024 进制） |
| `created` / `modified` / `accessed` | `modified: { older_than: 30d, newer_than: 1y }`（单位 s m h d w mo y，可组合如 `1d12h`） |
| `hidden` | `hidden: true` |
| `all` / `any` / `not` | 任意嵌套 |

**动作**

| 动作 | 说明 |
|---|---|
| `move: "目标文件夹"` | 文件名不变；相对路径以规则的 location 为基准 |
| `copy: "目标文件夹"` | 后续动作仍作用于原文件 |
| `rename: "新文件名"` | 在原文件夹内改名 |
| `trash` | 移入系统回收站，必须是最后一个动作 |

`move` / `copy` / `rename` 也可以写成完整形式：`{ to: "...", on_conflict: rename | skip | overwrite }`。

- `rename`（默认）：目标已存在时自动改成 `name (1).ext`
- `skip`：跳过这个文件
- `overwrite`：先把已有文件移入回收站，再放入新文件

**路径模板**：`{变量[:格式][|过滤器]}`

- 变量：`name` `stem` `ext` `parent` `size` `created` `modified` `accessed` `now` `regex.1` `regex.组名`
- 时间格式使用 strftime，默认为 `%Y-%m-%d`
- 过滤器：`lower` `upper` `slug` `trim`
- 变量值中的 `: * ? " < > |` 会被替换成 `_`，保证同一份规则在各平台上行为一致

## 安全机制

- `plan` 只读；`run` 默认要确认，操作超过 500 项时会额外提醒
- 不能把磁盘根目录、系统目录、`~/Library`、`AppData` 等设为整理位置，也不能递归整理整个用户主目录
- 执行时如果发现目标位置已有文件，绝不覆盖
- 删除只会移入回收站
- 每个操作都会写入日志，可以撤销；撤销前检查文件是否被改过，被改过的副本会保留。移入回收站的文件需要到回收站手动还原

## 开发

```bash
cargo test                       # 核心库和命令行
cargo clippy --all-targets
```

图形界面需要 Node.js 20 以上：

```bash
cd apps/desktop
npm ci
npm run tauri dev                # 开发模式，修改前端代码后自动刷新
npx tauri build --bundles dmg    # 本机打包（Windows 上用 --bundles nsis）
```

开发时可以用环境变量让规则和日志指向测试目录，避免影响真实文件：

```bash
FOLDO_RULES=/tmp/foldo-test/rules.yaml FOLDO_HOME=/tmp/foldo-test/data npm run tauri dev
```
