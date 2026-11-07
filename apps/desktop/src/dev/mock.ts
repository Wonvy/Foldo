// 开发预览用的模拟后端：在普通浏览器中打开 Vite 开发服务器时注入，返回逼真的示例数据。
// 只在 import.meta.env.DEV 且不在 Tauri 中时加载（见 main.tsx），正式构建不包含这个文件。
// 地址加 ?mock=empty 可以预览首次使用的引导页。
import type {
  Batch,
  Category,
  ConfigView,
  OpView,
  PlanView,
  RuleFile,
  SkipKind,
  SkipView,
  UndoCheck,
} from "../api";

const HOME = "/Users/demo";
const DL = `${HOME}/Downloads`;
const DESK = `${HOME}/Desktop`;
const disp = (p: string) => p.replace(HOME, "~");
const base = (p: string) => p.split("/").pop() ?? p;
const dir = (p: string) => p.slice(0, p.lastIndexOf("/"));

const CATEGORIES: Category[] = [
  { id: "image", name: "图片", aliases: ["Pictures"], extensions: ["jpg", "jpeg", "png", "gif", "heic", "webp", "svg"] },
  { id: "document", name: "文档", aliases: ["Documents"], extensions: ["pdf", "doc", "docx", "txt", "md", "pages"] },
  { id: "sheet", name: "表格与演示", aliases: [], extensions: ["xls", "xlsx", "csv", "ppt", "pptx", "key"] },
  { id: "archive", name: "压缩包", aliases: [], extensions: ["zip", "rar", "7z", "tar", "gz"] },
  { id: "installer", name: "安装包", aliases: [], extensions: ["dmg", "pkg", "exe", "msi"] },
  { id: "video", name: "视频", aliases: [], extensions: ["mp4", "mov", "mkv", "avi"] },
  { id: "audio", name: "音频", aliases: [], extensions: ["mp3", "wav", "m4a", "flac"] },
  { id: "design", name: "设计文件", aliases: [], extensions: ["psd", "ai", "sketch", "fig", "xd"] },
  { id: "model", name: "3D 模型", aliases: [], extensions: ["step", "stp", "stl", "3mf", "obj", "f3d", "dwg"] },
  { id: "code", name: "代码", aliases: [], extensions: ["py", "js", "ts", "rs", "json", "html"] },
  { id: "font", name: "字体", aliases: [], extensions: ["ttf", "otf", "woff2"] },
];
const catOf = (name: string) => {
  const ext = name.includes(".") ? name.split(".").pop()!.toLowerCase() : "";
  return CATEGORIES.find((c) => c.extensions.includes(ext)) ?? null;
};


type F = [string, string, string, string, string | null, string, string[]?];
const CAT_NAMES = ["图片", "文档", "表格与演示", "压缩包", "安装包", "视频", "音频", "设计文件", "3D 模型", "代码", "字体"];
const FIELD_ROWS: F[] = [
  ["name", "文件名", "基本", "text", null, "含扩展名"],
  ["stem", "主名", "基本", "text", null, "不含扩展名"],
  ["ext", "扩展名", "基本", "text", null, "不含点"],
  ["parent", "所在文件夹", "基本", "text", null, ""],
  ["path", "完整路径", "基本", "text", null, ""],
  ["size", "大小", "基本", "size", "B / KB / MB / GB", ""],
  ["category", "类型", "基本", "enum", null, "按扩展名归类", CAT_NAMES],
  ["mime", "真实格式", "基本", "text", null, "按文件头识别"],
  ["hidden", "隐藏", "基本", "bool", null, ""],
  ["readonly", "只读", "基本", "bool", null, ""],
  ["empty", "空文件", "基本", "bool", null, ""],
  ["created", "创建时间", "时间", "date", null, ""],
  ["modified", "修改时间", "时间", "date", null, ""],
  ["accessed", "访问时间", "时间", "date", null, ""],
  ["modified.year", "修改·年", "时间", "number", null, ""],
  ["modified.month", "修改·月", "时间", "number", null, ""],
  ["modified.weekday", "修改·星期", "时间", "number", null, "周一为 1"],
  ["image.width", "图片宽度", "图片", "number", "像素", ""],
  ["image.height", "图片高度", "图片", "number", "像素", ""],
  ["image.pixels", "总像素", "图片", "number", "百万像素", ""],
  ["image.ratio", "宽高比", "图片", "number", null, ""],
  ["image.orientation", "方向", "图片", "enum", null, "", ["横", "竖", "方"]],
  ["exif.taken", "拍摄时间", "照片 EXIF", "date", null, ""],
  ["exif.make", "相机品牌", "照片 EXIF", "text", null, ""],
  ["exif.model", "相机型号", "照片 EXIF", "text", null, ""],
  ["exif.lens", "镜头", "照片 EXIF", "text", null, ""],
  ["exif.iso", "ISO", "照片 EXIF", "number", null, ""],
  ["exif.gps", "有定位", "照片 EXIF", "bool", null, ""],
  ["video.duration", "视频时长", "视频", "number", "秒", ""],
  ["video.width", "视频宽度", "视频", "number", "像素", ""],
];
const FIELDS = FIELD_ROWS.map(([key, label, group, type, unit, desc, options]) => ({ key, label, group, type, unit, desc, options: options ?? [] }));
const VARS = [
  { key: "now:%Y-%m-%d", label: "当前时间", group: "时间", desc: "" },
  { key: "match.1", label: "正则捕获 1", group: "正则", desc: "" },
  { key: "dir.home", label: "用户主目录", group: "系统文件夹", desc: "" },
  { key: "dir.desktop", label: "桌面", group: "系统文件夹", desc: "" },
  { key: "dir.documents", label: "文稿", group: "系统文件夹", desc: "" },
  { key: "dir.downloads", label: "下载", group: "系统文件夹", desc: "" },
  { key: "dir.pictures", label: "图片", group: "系统文件夹", desc: "" },
  { key: "system.user", label: "用户名", group: "系统", desc: "" },
  { key: "system.os", label: "操作系统", group: "系统", desc: "" },
  { key: "env.HOME", label: "环境变量", group: "系统", desc: "" },
];
const SAMPLE_ATTRS: Record<string, string> = {
  name: "IMG_2034.HEIC", stem: "IMG_2034", ext: "HEIC", parent: "Downloads", size: "2480132 字节", category: "图片",
  mime: "image/heif", hidden: "否", readonly: "否", empty: "否", created: "2026-11-02 16:40:12", modified: "2026-11-02 16:40:12",
  "modified.year": "2026", "modified.month": "11", "image.width": "4032", "image.height": "3024", "image.pixels": "12.19",
  "image.ratio": "1.33", "image.orientation": "横", "exif.taken": "2026-11-02 16:39:58", "exif.make": "Apple",
  "exif.model": "iPhone 16 Pro", "exif.lens": "iPhone 16 Pro back triple camera 6.765mm f/1.78", "exif.iso": "80", "exif.gps": "是",
};
const explainAll = (d: unknown): unknown => {
  const [k, v] = Object.entries(d as Record<string, unknown>)[0];
  if (k === "all" || k === "any" || k === "none") {
    const children = (v as unknown[]).map(explainAll);
    return { ok: k !== "none", children };
  }
  if (k === "not") return { ok: false, children: [explainAll(v)] };
  return { ok: !k.startsWith("video") };
};

let config: RuleFile | null = new URLSearchParams(location.search).get("mock") === "empty"
  ? null
  : {
      version: 1,
      folders: [
        { path: DL },
        { path: DESK },
      ],
      protect: ["*.lic", `${DL}/重要合同`],
      settings: { buffer: "10m" },
      rules: [
        {
          name: "截图改名归档",
          locations: [DESK],
          conditions: [{ category: "图片" }, { name: { regex: "^(Screenshot|截屏)" } }],
          actions: [{ rename: "截图_{modified:%Y%m%d_%H%M%S}.{ext|lower}" }, { move: `${HOME}/Pictures/Screenshots/` }],
        },
        {
          name: "3D 模型归档",
          conditions: [{ category: "3D 模型" }],
          actions: [{ move: "3D 模型/" }],
        },
        {
          name: "发票归档",
          conditions: [{ name: { contains: "发票" } }],
          except: [{ category: "压缩包" }],
          actions: [{ move: { to: `${HOME}/Documents/发票`, on_conflict: "rename" } }],
        },
        {
          name: "文档归档",
          conditions: [{ category: "文档" }],
          actions: [{ move: "Documents/" }],
        },
        {
          name: "图片归档",
          conditions: [{ category: "图片" }],
          actions: [{ move: "Pictures/" }],
        },
        {
          name: "旧压缩包清理",
          enabled: false,
          locations: [DL],
          conditions: [{ category: "压缩包" }, { modified: { older_than: "30d" } }, { size: { lt: "10MB" } }],
          actions: ["trash"],
        },
      ],
    };

function view(): ConfigView {
  return {
    home: HOME,
    path: `${HOME}/Library/Application Support/foldo/rules.yaml`,
    display_path: "~/Library/Application Support/foldo/rules.yaml",
    exists: config !== null,
    config,
    error: null,
  };
}

type Planned = { name: string; from: string; to: string; reason: string; rename?: string; trash?: boolean };

const PLANNED: Planned[] = [
  { name: "旋钮工装.step", from: DL, to: `${DL}/3D 模型`, reason: "规则「3D 模型归档」" },
  { name: "旋钮工装.3mf", from: DL, to: `${DL}/3D 模型`, reason: "规则「3D 模型归档」" },
  { name: "支架装配_v3.stp", from: DL, to: `${DL}/3D 模型`, reason: "规则「3D 模型归档」" },
  { name: "2026Q3 经营报告.pdf", from: DL, to: `${DL}/Documents`, reason: "规则「文档归档」" },
  { name: "需求说明 v2.docx", from: DL, to: `${DL}/Documents`, reason: "规则「文档归档」" },
  { name: "发票-2026-10-京东.pdf", from: DL, to: `${HOME}/Documents/发票`, reason: "规则「发票归档」" },
  { name: "IMG_2034.HEIC", from: DL, to: `${DL}/Pictures`, reason: "规则「图片归档」" },
  { name: "产品主图-白底.png", from: DL, to: `${DL}/Pictures`, reason: "规则「图片归档」" },
  { name: "截屏2026-11-06 10.21.33.png", from: DESK, to: `${HOME}/Pictures/Screenshots`, reason: "规则「截图改名归档」", rename: "截图_20261106_102133.png" },
];

const SKIPPED: [string, SkipKind, string][] = [
  [`${DL}/项目说明`, "no_match", "没有匹配的规则，保持原位"],
  [`${DESK}/Figma 导出-首页.fig`, "no_match", "没有匹配的规则，保持原位"],
  [`${DL}/OrcaSlicer-2.3.dmg`, "no_match", "没有匹配的规则，保持原位"],
  [`${DL}/素材包-图标.zip`, "no_match", "没有匹配的规则，保持原位"],
  [`${DL}/测量数据.parquet`, "no_match", "没有匹配的规则，保持原位"],
  [`${DESK}/待办`, "no_match", "没有匹配的规则，保持原位"],
  [`${DL}/季度报告.pdf`, "conflict", "目标位置已有同名文件：~/Downloads/Documents/季度报告.pdf"],
  [`${DL}/项目A-机械臂`, "folder", "已有文件夹作为整体保留，不拆分内部文件"],
  [`${DL}/客户资料`, "folder", "已有文件夹作为整体保留，不拆分内部文件"],
  [`${DL}/重要合同`, "protected", "在固定保留列表中（~/Downloads/重要合同）"],
  [`${DL}/server.pem`, "credential", "可能是私钥、证书或密码库，默认不移动"],
  [`${DESK}/Chrome.lnk`, "shortcut", "快捷方式默认保留在原位"],
  [`${DL}/.DS_Store`, "system", "系统自动生成的文件"],
  [`${DL}/纪录片.mp4.crdownload`, "partial", "正在下载或被 Office 打开的临时文件"],
  [`${DL}/刚下载的图.png`, "buffer", "最近 10m 内新增或修改，暂不整理"],
];

const LABEL: Record<SkipKind, string> = {
  protected: "固定保留",
  credential: "凭证文件",
  shortcut: "快捷方式",
  system: "系统文件",
  hidden: "隐藏文件",
  partial: "下载未完成",
  buffer: "新文件缓冲期",
  in_use: "正在使用",
  folder: "文件夹",
  no_match: "没有匹配的规则",
  conflict: "目标冲突",
  invalid: "无法应用规则",
};

function skip(path: string, kind: SkipKind, reason: string): SkipView {
  return { path, name: base(path), display: disp(path), kind, label: LABEL[kind], reason, rule: null };
}

function op(group: number, p: Planned, kind: OpView["kind"], src: string, dst: string | null): OpView {
  const origin = `${p.from}/${p.name}`;
  return {
    group,
    origin,
    origin_name: p.name,
    origin_display: disp(origin),
    rule: p.reason,
    reason: p.reason,
    category: catOf(p.name)?.name ?? null,
    kind,
    src,
    src_display: disp(src),
    dst,
    dst_display: dst && disp(dst),
    dst_dir: dst && dir(dst),
    dst_dir_display: dst && disp(dir(dst)),
    note: null,
  };
}

let seq = 0;
let current: PlanView | null = null;

function makePlan(list: Planned[], skipped: SkipView[]): PlanView {
  const ops: OpView[] = [];
  list.forEach((p, g) => {
    const origin = `${p.from}/${p.name}`;
    if (p.rename) {
      const renamed = `${p.from}/${p.rename}`;
      ops.push(op(g, p, "rename", origin, renamed));
      ops.push(op(g, p, "move", renamed, `${p.to}/${p.rename}`));
    } else if (p.trash) {
      ops.push(op(g, p, "trash", origin, null));
    } else {
      ops.push(op(g, p, "move", origin, `${p.to}/${p.name}`));
    }
  });
  current = {
    plan_id: ++seq,
    groups: list.length,
    ops,
    skipped,
    warnings: [],
    scanned: list.length + skipped.length,
    matched: list.length + 1,
    in_place: 0,
  };
  return current;
}

const batches: Batch[] = [
  { id: 3, started_at: "2026-11-06 18:20:11", finished_at: "2026-11-06 18:20:12", source: "gui", status: "done", op_count: 14 },
  { id: 2, started_at: "2026-11-05 09:02:47", finished_at: "2026-11-05 09:02:47", source: "gui", status: "partial", op_count: 6 },
  { id: 1, started_at: "2026-11-03 21:14:05", finished_at: "2026-11-03 21:14:05", source: "cli", status: "undone", op_count: 9 },
];

const sleep = (ms: number) => new Promise((r) => setTimeout(r, ms));

async function handle(cmd: string, args: Record<string, unknown>): Promise<unknown> {
  switch (cmd) {
    case "get_config":
      return view();
    case "save_config":
      config = args.config as RuleFile;
      return view();
    case "add_protect":
      config = { ...(config ?? { version: 1 }), protect: [...(config?.protect ?? []), args.entry as string] };
      return view();
    case "get_yaml":
      return `# Foldo 配置文件（模拟数据）\nversion: 1\nfolders:\n  - path: ~/Downloads\n    categories: true\n`;
    case "save_yaml":
      return view();
    case "categories":
      return CATEGORIES;
    case "suggested_folders":
      return [
        { path: DL, display: "~/Downloads", label: "下载" },
        { path: DESK, display: "~/Desktop", label: "桌面" },
        { path: `${HOME}/Documents`, display: "~/Documents", label: "文稿" },
      ];
    case "scan":
      await sleep(450);
      return makePlan(PLANNED, SKIPPED.map(([p, k, r]) => skip(p, k, r)));
    case "scan_paths": {
      await sleep(300);
      const paths = args.paths as string[];
      return makePlan(
        [],
        paths.map((p) => skip(p, "no_match", "没有匹配的规则，保持原位")),
      );
    }
    case "try_rule":
      await sleep(250);
      return {
        ...makePlan(PLANNED.slice(0, 3), []),
        matched: 3,
      };
    case "retarget": {
      if (!current) throw "预览已过期，请重新扫描";
      const g = args.group as number;
      const target = args.dir as string;
      const first = current.ops.find((o) => o.group === g)!;
      const dst = `${target}/${first.origin_name}`;
      current = {
        ...current,
        ops: current.ops
          .filter((o) => o.group !== g || o === first)
          .map((o) =>
            o === first
              ? {
                  ...o,
                  kind: "move" as const,
                  src: o.origin,
                  src_display: o.origin_display,
                  dst,
                  dst_display: disp(dst),
                  dst_dir: target,
                  dst_dir_display: disp(target),
                  reason: "手动指定位置",
                  rule: "手动指定",
                }
              : o,
          ),
      };
      return current;
    }
    case "execute_plan": {
      await sleep(500);
      const groups = args.groups as number[];
      const n = current?.ops.filter((o) => groups.includes(o.group)).length ?? 0;
      batches.unshift({
        id: batches[0].id + 1,
        started_at: "2026-11-07 12:00:00",
        finished_at: "2026-11-07 12:00:01",
        source: "gui",
        status: "done",
        op_count: n,
      });
      return {
        batch_id: batches[0].id,
        done: n - 1,
        skipped: [{ path: "~/Downloads/需求说明 v2.docx", name: "需求说明 v2.docx", error: "目标位置出现了同名文件，为避免覆盖已跳过" }],
        failed: [],
      };
    }
    case "history":
      return batches;
    case "batch_ops":
      return PLANNED.slice(0, 5).map((p, i) => ({
        id: i,
        seq: i,
        rule: p.reason,
        kind: "move",
        name: p.name,
        src: `${p.from}/${p.name}`,
        dst: `${p.to}/${p.name}`,
        src_display: disp(`${p.from}/${p.name}`),
        dst_display: disp(`${p.to}/${p.name}`),
        status: i === 4 ? "skipped" : "done",
        error: i === 4 ? "目标位置出现了同名文件，为避免覆盖已跳过" : null,
      }));
    case "undo_preview":
      return PLANNED.slice(0, 6).map(
        (p, i): UndoCheck => ({
          kind: "move",
          name: p.name,
          src_display: disp(`${p.from}/${p.name}`),
          dst_display: disp(`${p.to}/${p.name}`),
          outcome: i === 2 ? "blocked" : "restore",
          reason: i === 2 ? "文件已不在 ~/Downloads/3D 模型/支架装配_v3.stp，可能被移走或删除" : null,
        }),
      );
    case "undo": {
      const b = batches.find((x) => x.id === args.id);
      if (b) b.status = "partially_undone";
      return { batch_id: args.id, restored: 5, failed: [{ path: "~/Downloads/3D 模型/支架装配_v3.stp", name: "支架装配_v3.stp", error: "文件已不在原处" }], in_trash: [] };
    }
    case "fields":
      return { fields: FIELDS, vars: VARS };
    case "inspect":
      return FIELDS.map((f) => ({ key: f.key, label: f.label, group: f.group, unit: f.unit, value: SAMPLE_ATTRS[f.key] ?? null }));
    case "test_rule": {
      await sleep(200);
      const rule = args.rule as { when?: unknown; except?: unknown[] };
      return {
        matched: true,
        conditions: [],
        when: rule.when ? explainAll(rule.when) : null,
        exceptions: (rule.except ?? []).map(() => ({ ok: false })),
        destination: `${HOME}/Pictures/2026/11/IMG_2034.HEIC`,
        reason: null,
      };
    }
    case "plugin:dialog|open":
      return (args.options as { directory?: boolean })?.directory ? `${HOME}/Documents/精选` : `${DL}/IMG_2034.HEIC`;
    case "debug_file": {
      const path = args.path as string;
      const rules = config?.rules ?? [];
      const winner = rules.findIndex((r) => r.enabled !== false && JSON.stringify(r).includes("图片"));
      return {
        path,
        name: base(path),
        display: disp(path),
        category: catOf(base(path))?.name ?? null,
        excluded: null,
        rules: rules.map((r, index) => ({ index, name: r.name, enabled: r.enabled !== false, matched: JSON.stringify(r).includes("图片"), in_scope: true })),
        winner: winner >= 0 ? winner : null,
        action: winner >= 0 ? "移动到" : null,
        destination: winner >= 0 ? `${HOME}/Pictures/${base(path)}` : null,
        reason: winner >= 0 ? null : "没有匹配的规则，保持原位",
      };
    }
    case "toggle_float":
    case "float_visible":
      return false;
    case "float_resize":
    case "show_main":
    case "plugin:window|start_dragging":
      return null;
    case "reveal":
      return null;
    case "plugin:window|set_theme":
    case "plugin:event|listen":
    case "plugin:event|unlisten":
      return 1;
    default:
      throw `模拟后端未实现：${cmd}`;
  }
}

export function installMock() {
  let cb = 0;
  (window as unknown as Record<string, unknown>).__TAURI_INTERNALS__ = {
    invoke: (cmd: string, args: Record<string, unknown> = {}) => handle(cmd, args),
    transformCallback: () => ++cb,
    unregisterCallback: () => {},
    convertFileSrc: (p: string) => p,
    metadata: {
      currentWindow: { label: "main" },
      currentWebview: { label: "main", windowLabel: "main" },
    },
  };
  (window as unknown as Record<string, unknown>).__TAURI_EVENT_PLUGIN_INTERNALS__ = {
    unregisterListener: () => {},
  };
  console.info("[Foldo] 已启用模拟后端（仅开发预览）");
}
