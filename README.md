# Foldo

[![Build](https://github.com/Wonvy/Foldo/actions/workflows/build.yml/badge.svg)](https://github.com/Wonvy/Foldo/actions/workflows/build.yml)

跨平台（macOS / Windows / Linux）的文件夹整理工具：用 YAML 写规则，先预览、再执行，执行过的操作可以撤销。

提供图形界面（Tauri）和命令行两种用法，两者共用同一份规则文件和操作日志。

**怎么用**：在“规则”页新建规则，写明哪些文件怎么处理：条件可以是类型、扩展名、文件名、大小、时间、图片尺寸、是否重复等，还可以加例外；动作可以是移动、复制、改名、解压、压缩、创建快捷方式、移到回收站或保持原位。然后在“整理”页给每个文件夹添加要用的规则。扫描后先看整理建议，勾选后执行，随时可以撤销；也可以让 Foldo 在后台监听文件夹自动整理，或把文件拖进窗口按规则处理（类似 DropIt）。

Foldo 不会自己推断规则，也不会按类型自动整理：只有你建的规则会移动文件，没有规则命中的文件保持原位。

判断顺序：固定保留与排除 → 规则（从上到下，第一条命中的生效）。只整理所选文件夹第一层的散落文件，已有文件夹整体保留；凭证文件、快捷方式、系统和隐藏文件、下载中或被占用的文件、缓冲期内的新文件都不会被移动。

当前进度：M1 核心引擎 + CLI、M2 图形界面已完成。之后是自动监听与定时运行。

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
   version: 2
   folders:
     - path: ~/FoldoTest/Inbox
       rules: [图片归档, 文档归档]
   rules:
     - name: 图片归档
       conditions:
         - extension: [jpg, png]
       actions:
         - move: "~/FoldoTest/图片/{modified:%Y}/{modified:%m}/"
     - name: 文档归档
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

第一次打开时会让你选择要整理的文件夹。试用时**建议只勾选一个测试文件夹**（点“选择其他文件夹…”，例如事先建好的 `FoldoTest`），再到“规则”页新建一两条规则，回到“整理”页把规则添加到这个文件夹，看清楚建议后再执行。

### 发布新版本

提交后打标签并推送，GitHub 会自动构建并创建 Release（命令行版的版本号在 `Cargo.toml` 里修改）：

```bash
git tag v0.1.0
git push origin v0.1.0
```

带连字符的标签（如 `v0.2.0-beta.1`）会被标记为预发布版本。图形界面的版本号直接取自标签，不需要另外修改。

### 自动更新

已安装的 Foldo 启动时会检查 GitHub 上最新的正式版（不含预发布版本），有新版本时“设置”旁边会出现“新”，在“设置 → 版本与更新”里一键下载并重启。目前支持 Windows 和 macOS（Apple 芯片）；Linux 的 .deb 需要手动下载安装。

更新包需要签名，第一次发布前要在 GitHub 上配置一次签名私钥：

1. 私钥在本机 `~/.tauri/foldo-updater.key`（对应的公钥已经写在 `apps/desktop/src-tauri/tauri.conf.json` 里）。**私钥不要提交到仓库，也不要发给别人；弄丢后已安装的版本就无法再自动更新。**
2. 打开仓库的 Settings → Secrets and variables → Actions → New repository secret，名称填 `TAURI_SIGNING_PRIVATE_KEY`，值填私钥文件的全部内容（macOS 上可用 `pbcopy < ~/.tauri/foldo-updater.key` 复制）。
3. 之后推送 `v*` 标签时，Release 里会多出签名文件和 `latest.json`，自动更新就会生效。

没有配置这个密钥时，构建照常进行，只是不生成更新包。

## 从源码构建

```bash
cargo build --release
./target/release/foldo init        # 生成示例规则
./target/release/foldo check       # 校验规则
./target/release/foldo plan        # 预览，不改动任何文件
./target/release/foldo run         # 预览并确认后执行（-y 跳过确认）
./target/release/foldo history     # 执行历史
./target/release/foldo undo [批次]  # 撤销，默认最近一次
./target/release/foldo inspect 文件  # 查看一个文件的全部属性（写规则时参考）
```

规则文件默认位于系统配置目录下的 `foldo/rules.yaml`（macOS：`~/Library/Application Support/foldo/rules.yaml`），可以用 `--rules` 或环境变量 `FOLDO_RULES` 指定。操作日志默认放在系统数据目录下的 `foldo/`，可以用 `--data-dir` 或 `FOLDO_HOME` 指定。

## 配置文件参考

图形界面的“整理”“规则”“设置”页面会读写这个文件；也可以直接编辑（图形界面里在“设置 → 编辑源码”）。

规则只描述“什么样的文件 → 怎么处理”；每个整理文件夹再指定使用哪些规则。没有指定规则的文件夹不整理任何文件。

```yaml
version: 2
folders:                     # 整理的文件夹
  - path: ~/Downloads
    rules: [规则名, 另一条规则] # 这个文件夹使用的规则；执行顺序以下面 rules 的顺序为准
    recursive: false         # 是否连子文件夹里的文件一起整理
    include_hidden: false    # 是否处理隐藏文件
protect:                     # 固定保留：路径或文件名通配符
  - ~/Downloads/重要合同
  - "*.lic"
settings:
  buffer: 10m                # 新文件缓冲期
  auto: notify               # 后台自动整理：off（默认）/ notify 发现文件时提醒 / run 直接整理（移到回收站仍需确认）
  keep_empty_folders: false  # true 时保留整理后变空的文件夹（默认删除，撤销时重建）
rules:
  - name: 规则名（唯一）
    enabled: true            # 默认 true
    when: { all: [...] }     # 条件树，可任意嵌套 all / any / none / not；不写则匹配所有文件
    except: [...]            # 例外：命中任意一条则不适用
    actions: [...]
```

旧的 `version: 1` 写法（规则里写 `locations` / `recursive`）读取时会自动转换：规则加入对应的文件夹，没写 `locations` 的规则加入全部文件夹，整理结果不变。图形界面保存时会写成新写法。

**执行语义**

- 规则从上到下依次生效，每个文件只由第一条命中它的规则处理。
- 一条规则里的多个动作依次作用于文件当前的位置，比如先 `rename` 再 `move`。
- 某个文件的动作链中只要有一步无法完成，这个文件就整体跳过，不会只做一半。

**条件**

每个条件是 `字段: { 运算符: 值 }`，条件之间用组组合：

```yaml
when:
  all:                                   # 全部满足（与）
    - category: 图片                       # 简写：等于
    - ext: [jpg, heic]                     # 简写：在列表中
    - image.width: { ">=": 3000 }
    - size: { gt: 1MB, lt: 1GB }           # 同一字段多个运算符：都要满足
    - any:                                 # 任一满足（或）
        - exif.model: { contains: iPhone }
        - name: { matches: '^IMG_(?P<year>\d{4})', case_sensitive: true }
    - none:                                # 都不满足
        - name: { contains: 草稿 }
    - not: { exif.gps: true }              # 取反
    - parent: { eq: "{modified.year}" }    # 比较值里可以写变量
```

| 字段类型 | 运算符 |
|---|---|
| 文本 | `eq` `ne` `contains` `not_contains` `starts_with` `ends_with` `in` `not_in` `glob` `matches`（正则） `empty`；默认不区分大小写，`case_sensitive: true` 可切换 |
| 数字 / 大小 | `eq` `ne` `gt` `gte` `lt` `lte`（也可写 `= != > >= < <=`） `between: [最小, 最大]`；大小可带单位 `10MB`（KB 按 1000 进制，KiB 按 1024），时长可写 `90`、`5m`、`1h` |
| 日期 | `before` `after` `on` `between`（如 `2026-01-31`） `older_than` `newer_than`（`30d` `12h` `2w` `6mo`） `within`（`today` `yesterday` `this_week` `last_week` `this_month` `last_month` `this_year` `last_year`，也可写中文） |
| 枚举 / 是否 | `eq` `ne` `in` `not_in` / `true` `false` |
| 所有字段 | `exists: true / false`（例如“有 EXIF 拍摄时间”） |

可用字段（`foldo inspect <文件>` 可以列出某个文件的全部取值）：

| 分组 | 字段 |
|---|---|
| 基本 | `name` `stem` `ext` `parent` `path` `size` `category` `mime`（按文件头识别的真实格式） `hidden` `readonly` `empty` `duplicate`（同一整理文件夹里有内容完全相同的文件，最早修改的一份不算） `duplicate.of`（原件文件名） |
| 时间 | `created` `modified` `accessed`，以及分量 `modified.year` `.month` `.day` `.weekday`（周一为 1） `.hour`（`created.*` `accessed.*` 同理） |
| 图片 | `image.width` `image.height` `image.pixels`（百万像素） `image.ratio` `image.orientation`（横 / 竖 / 方） |
| 照片 EXIF | `exif.taken` `exif.make` `exif.model` `exif.lens` `exif.software` `exif.iso` `exif.focal` `exif.gps` |
| 视频 | `video.duration` `video.width` `video.height` `video.taken` `video.model` |
| 音频 | `audio.duration` `audio.bitrate` `audio.title` `audio.artist` `audio.album` `audio.genre` `audio.year` |
| PDF | `pdf.pages` `pdf.title` `pdf.author` |
| Office 文档 | `doc.title` `doc.author` `doc.company` `doc.pages` `doc.slides` |
| 内容 | `content`（txt、md、csv、json 等纯文本的前 1 MB） |
| 下载来源 | `source.url` `source.domain`（macOS 的“来源”、Windows 的下载标记、Linux 的 xdg 属性） |

元数据按需读取：只有规则用到的字段才会去读，同一个文件在一次扫描中每类只读一次。旧写法（`extension`、`name: "*.pdf"`、`name: { regex: … }`、`conditions:` + `match:`）仍然有效。

**动作**

| 动作 | 说明 |
|---|---|
| `move: "目标文件夹"` | 文件名不变；相对路径以文件所在的整理文件夹为基准 |
| `copy: "目标文件夹"` | 后续动作仍作用于原文件 |
| `rename: "新文件名"` | 在原文件夹内改名 |
| `extract: "{stem}/"` | 解压 zip 到文件夹（相对压缩包所在位置），原文件保留 |
| `compress: "{stem}.zip"` | 压缩成 zip（相对文件所在位置），原文件保留 |
| `shortcut: "~/Desktop/常用/"` | 在目标文件夹创建快捷方式（Windows 为 .lnk，其他系统为符号链接），原文件保留 |
| `trash` | 移入系统回收站，必须是最后一个动作 |
| `keep` | 保持原位（“这类文件别动”），只能单独使用 |

复制、解压、压缩、快捷方式的结果已经存在时视为已完成，不会每次扫描都再生成一份。撤销时：副本和快捷方式直接删除，解压出的文件夹和压缩包放进回收站。

`move` / `copy` / `rename` / `extract` / `compress` / `shortcut` 也可以写成完整形式：`{ to: "...", on_conflict: rename | skip | overwrite }`。

- `rename`（默认）：目标已存在时自动改成 `name (1).ext`
- `skip`：跳过这个文件
- `overwrite`：先把已有文件移入回收站，再放入新文件

**路径模板**：`{变量[:时间格式] [?? 备选] [|过滤器]}`，例如 `{dir.pictures}/{exif.taken ?? modified:%Y}/{exif.model|slug}/`

- 变量：上面所有字段，以及 `now`、`match.1` / `match.组名`（条件里正则的捕获组）、`dir.home|desktop|documents|downloads|pictures|music|videos`（跨平台的系统文件夹）、`system.user|host|os`、`env.任意环境变量`
- 时间格式使用 strftime，默认为 `%Y-%m-%d`
- 备选：`{exif.taken ?? created ?? modified:%Y}`，也可以写死 `{exif.model ?? "未知相机"}`
- 过滤器：`lower` `upper` `title` `slug` `trim` `pad(3)` `slice(0,8)` `replace("正则","替换")` `default("值")`
- 文件属性变量的值中 `: * ? " < > |` 会被替换成 `_`，保证同一份规则在各平台上行为一致；`dir.*`、`env.*` 是路径，原样使用

## 安全机制

- `plan` 只读；`run` 默认要确认，操作超过 500 项时会额外提醒
- 不能把磁盘根目录、系统目录、`~/Library`、`AppData` 等设为整理位置，也不能递归整理整个用户主目录
- 执行时如果发现目标位置已有文件，绝不覆盖
- 删除只会移入回收站
- 执行时才出现的新冲突（目标位置刚出现同名文件、源文件已被移走）会跳过并提示，不算失败
- 每个操作都会写入日志，可以撤销；撤销前先给出撤销预览，列出能恢复的项目和不能恢复的原因。被改过的副本会保留；移入回收站的文件需要到回收站手动还原

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
