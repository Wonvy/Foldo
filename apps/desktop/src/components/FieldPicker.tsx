// 属性选择器：图标 + 名称的小按钮，点开是按分组列出、可搜索的属性列表。
import {
  Camera,
  ChevronDown,
  Clock,
  File,
  FileMusic,
  FileSpreadsheet,
  FileText,
  Film,
  Globe,
  Image,
  Search,
  Type,
  type LucideIcon,
} from "lucide-react";
import { useRef, useState } from "react";
import type { FieldDef } from "../api";
import Popover from "./Popover";

export const GROUP_ICON: Record<string, LucideIcon> = {
  基本: File,
  时间: Clock,
  图片: Image,
  "照片 EXIF": Camera,
  视频: Film,
  音频: FileMusic,
  PDF: FileText,
  "Office 文档": FileSpreadsheet,
  内容: Type,
  下载来源: Globe,
};

export function GroupIcon({ group, size = 14 }: { group: string; size?: number }) {
  const Icon = GROUP_ICON[group] ?? File;
  return <Icon size={size} />;
}

export default function FieldPicker({
  value,
  fields,
  onChange,
}: {
  value: string;
  fields: FieldDef[];
  onChange: (key: string) => void;
}) {
  const btn = useRef<HTMLButtonElement>(null);
  const [open, setOpen] = useState(false);
  const [q, setQ] = useState("");
  const current = fields.find((f) => f.key === value);
  const shown = fields.filter((f) => !q || `${f.label}${f.key}${f.group}${f.desc}`.toLowerCase().includes(q.toLowerCase()));
  const groups = [...new Set(shown.map((f) => f.group))];

  return (
    <>
      <button ref={btn} className="field-pill" title={current?.desc || current?.key} onClick={() => setOpen(!open)}>
        <GroupIcon group={current?.group ?? "基本"} />
        <span>{current?.label ?? value}</span>
        <ChevronDown size={13} className="chev" />
      </button>
      {open && (
        <Popover anchor={btn} onClose={() => setOpen(false)} width={320}>
          <div className="search-box">
            <Search size={14} />
            <input autoFocus placeholder="搜索属性" value={q} onChange={(e) => setQ(e.target.value)} />
          </div>
          <div className="pick-list">
            {groups.map((g) => (
              <div key={g}>
                <div className="pick-group">
                  <GroupIcon group={g} size={13} />
                  {g}
                </div>
                {shown
                  .filter((f) => f.group === g)
                  .map((f) => (
                    <button
                      key={f.key}
                      className={f.key === value ? "on" : ""}
                      title={f.desc}
                      onClick={() => {
                        onChange(f.key);
                        setOpen(false);
                        setQ("");
                      }}
                    >
                      {f.label}
                      {f.unit && !f.unit.includes("/") && <span className="muted">{f.unit}</span>}
                    </button>
                  ))}
              </div>
            ))}
            {shown.length === 0 && <div className="muted" style={{ padding: 12 }}>没有匹配的属性</div>}
          </div>
        </Popover>
      )}
    </>
  );
}
