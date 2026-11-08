import { defaultStyles, type FileIconProps, type IconType } from "react-file-icon";

// The library's presets take priority. These families cover Foldo's other built-in types.
const families: { category: string; type: IconType; color: string; extensions: string }[] = [
  {
    category: "图片",
    type: "image",
    color: "#c66b1b",
    extensions: "jpg jpeg png gif heic heif webp bmp tif tiff svg ico avif raw cr2 cr3 nef arw dng orf rw2",
  },
  { category: "文档", type: "document", color: "#4269ad", extensions: "pdf doc docx txt md rtf odt pages wps epub mobi" },
  { category: "表格与演示", type: "spreadsheet", color: "#25805c", extensions: "xls xlsx xlsm csv numbers ods et" },
  { category: "表格与演示", type: "presentation", color: "#bf623d", extensions: "ppt pptx key odp dps" },
  { category: "压缩包", type: "compressed", color: "#9f7a27", extensions: "zip rar 7z tar gz tgz bz2 xz zst" },
  { category: "安装包", type: "settings", color: "#8163b0", extensions: "dmg pkg exe msi msix deb rpm appimage apk" },
  { category: "视频", type: "video", color: "#ac5283", extensions: "mp4 mov avi mkv wmv flv webm m4v mpg mpeg" },
  { category: "音频", type: "audio", color: "#328e86", extensions: "mp3 wav flac aac m4a ogg wma aiff opus" },
  { category: "设计文件", type: "vector", color: "#9e58a8", extensions: "psd ai sketch fig xd indd cdr afdesign afphoto eps" },
  {
    category: "3D 模型",
    type: "3d",
    color: "#6866ad",
    extensions: "step stp stl 3mf obj fbx iges igs f3d sldprt sldasm slddrw blend glb gltf 3ds dwg dxf x_t skp",
  },
  {
    category: "代码",
    type: "code",
    color: "#526f8d",
    extensions: "py js ts jsx tsx rs go java kt c cpp h hpp cs swift rb php sh ps1 bat json yaml yml toml xml html css sql ipynb",
  },
  { category: "字体", type: "font", color: "#9e7a27", extensions: "ttf otf woff woff2 ttc" },
];
const byExtension = new Map(families.flatMap((family) => family.extensions.split(" ").map((ext) => [ext, family] as const)));
const presets: Record<string, Partial<FileIconProps>> = defaultStyles;
const configNames = new Set([".gitignore", ".gitattributes", ".editorconfig", ".env", "dockerfile", "makefile"]);

export function fileIconStyle(filename: string, category?: string | null): FileIconProps {
  const name = filename.split(/[\\/]/).pop()?.toLowerCase() ?? "";
  const dot = name.lastIndexOf(".");
  const extension = dot > 0 && dot < name.length - 1 ? name.slice(dot + 1) : "";
  const family = byExtension.get(extension) ?? families.find((family) => family.category === category);
  const color = family?.color ?? "#808896";
  return {
    color: "#f0f2f5",
    glyphColor: color,
    labelColor: color,
    labelTextColor: "#ffffff",
    type: configNames.has(name) ? "settings" : (family?.type ?? "document"),
    ...presets[extension],
    extension: extension.length > 5 ? extension.slice(0, 4) + "…" : extension,
    labelUppercase: true,
    gradientOpacity: 0,
  };
}
