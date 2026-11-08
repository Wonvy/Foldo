# 配置参考

[English overview](../README.md) · [中文说明](../README.zh-CN.md)

## 配置文件参考

图形界面的“整理”“规则”“设置”页面会读写这个文件；也可以直接编辑（图形界面里在“设置 → 编辑源码”）。

在“设置 → 文件类型与扩展名”中，可以修改图片、文档等默认类型的扩展名列表，也可以恢复默认。扩展名用逗号或空格分隔，不区分大小写；同一个扩展名只能属于一种类型。保存后重新扫描即可生效，规则测试和文件属性也会使用新分类。对应配置为 `settings.category_extensions`，以类型 ID 为键（例如 `image: [jpg, jpeg, png, jxl]`）；已填写的列表替换该类型的默认列表，空列表表示清空，未填写的类型仍使用默认值。

规则只描述“什么样的文件 → 怎么处理”；每个整理文件夹再指定使用哪些规则。没有指定规则的文件夹不整理任何文件。

```yaml
version: 2
folders:                     # 整理的文件夹
  - path: ~/Downloads
    rules: [规则名, 另一条规则] # 这个文件夹使用的规则；按此处的排列顺序匹配
    recursive: false         # 是否连子文件夹里的文件一起整理
    include_hidden: false    # 是否处理隐藏文件
protect:                     # 固定保留：路径或文件名通配符
  - ~/Downloads/重要合同
  - "*.lic"
settings:
  buffer: 10m                # 新文件缓冲期
  auto: notify               # 后台自动整理：off（默认）/ notify 发现文件时提醒 / run 直接整理（移到回收站仍需确认）
  keep_empty_folders: false  # true 时保留整理后变空的文件夹（默认删除，撤销时重建）
  keep_history_days: 365     # 历史记录保留天数，更早的自动清理（清理后不能撤销）；0 表示一直保留
  schedule:                  # 定时整理（可选）：到点整理一次，只做可以撤销的操作；关机错过的时间点下次打开时补上
    every: weekly            # daily 每天 / weekly 每周
    weekday: 1               # 每周时必填：1 = 周一 … 7 = 周日
    at: "18:30"              # 24 小时制
profile: 工作                # 当前配置方案（可选）
profiles:                    # 配置方案：保存起来的几套“文件夹 → 规则”，一键切换；当前生效的是上面的 folders
  - name: 个人
    folders:
      - path: ~/Pictures
        rules: [规则名]
rules:
  - name: 规则名（唯一）
    enabled: true            # 默认 true
    when: { all: [...] }     # 条件树，可任意嵌套 all / any / none / not；不写则匹配所有文件
    except: [...]            # 例外：命中任意一条则不适用
    actions: [...]
```

配置方案可以在整理页文件夹列表上方或托盘菜单里切换。切换时，当前的 `folders` 存回当前方案，再换成目标方案的文件夹设置；目标方案里引用了已删除的规则会被去掉。没有方案时第一次新建方案，原来的设置会保存成“默认方案”。

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
| 基本 | `name` `stem` `ext` `parent` `path` `size` `category` `mime`（按文件头识别的真实格式） `hidden` `readonly` `empty` `duplicate`（全部整理文件夹里有内容完全相同的文件，跨文件夹比较；最早修改的一份不算） `duplicate.of`（原件文件名） |
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
| `extract: "{stem}/"` | 解压 zip、7z、rar、tar、tar.gz 到文件夹（相对压缩包所在位置），原文件保留；压缩包里越出目标文件夹的路径会被跳过 |
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

**处理文件夹**：规则写 `target: folder` 时，处理的是整理文件夹**第一层的子文件夹**，整个文件夹一起移动、复制、改名、压缩、创建快捷方式或移入回收站（不能解压）。没有文件夹规则命中的子文件夹整体保持原位；被文件夹规则处理的子文件夹，里面的文件不再由文件规则单独处理。

```yaml
- name: 照片集归档
  target: folder
  when:
    all:
      - folder.content: 图片                    # 第一层文件里超过一半是图片
      - folder.names: { not_contains: 草稿 }    # 没有任何项目名包含“草稿”
  actions:
    - move: "{dir.pictures}/相册/"
```

| 字段 | 说明 |
|---|---|
| `folder.kind` | 文件夹类型：代码工程（有 `.git`、`package.json`、`.sln` 等）、应用程序（`.app`）、空文件夹、普通文件夹 |
| `folder.content` | 内容类型：第一层文件里超过一半是同一类型时为该类型，否则为“混合”；按自定义的文件类型统计 |
| `folder.names` | 第一层的文件和文件夹名（列表）：任意一项满足即可；`ne` / `not_contains` / `not_in` 表示没有任何一项满足 |
| `folder.exts` | 第一层文件的扩展名（列表，小写） |
| `folder.items` / `folder.files` / `folder.folders` | 第一层的项目数 / 文件数 / 子文件夹数 |
| `folder.changed` | 第一层里最近一次修改的时间 |
| `size`、`empty` 等 | 对文件夹分别表示总大小、里面是否什么都没有 |

刚建好或里面刚有改动的文件夹（可能还在下载或解压）会等过了新文件缓冲期再处理。

**界面语言**：图形界面支持简体中文、English、繁體中文、日本語，默认跟随系统语言，可在“设置 → 通用 → 语言”切换；后台通知和托盘菜单使用同一种语言。

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
