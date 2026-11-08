// 整理建议列表：每组一张卡片（按目标位置或整理原因），每个文件可以勾选。
// 文件很多时（几千个）只渲染屏幕附近的行，滚动时再换上，界面不会卡。
import { useVirtualizer } from "@tanstack/react-virtual";
import { ArrowRight, Ellipsis, Folder, Trash } from "lucide-react";
import { useLayoutEffect, useMemo, useRef, useState } from "react";
import { t } from "../../i18n";
import { Checkbox, FileIcon } from "../ui";
import type { Item, Section } from "./items";

/** 超过这么多条时改用虚拟列表。 */
const VIRTUAL_MIN = 200;

type Props = {
  sections: Section[];
  selected: Set<number>;
  groupBy: "target" | "rule";
  onToggle: (groups: number[], on: boolean) => void;
  onMenu: (el: HTMLElement, it: Item) => void;
};

export default function ItemSections(props: Props) {
  const total = props.sections.reduce((n, s) => n + s.items.length, 0);
  return total > VIRTUAL_MIN ? <VirtualSections {...props} /> : <PlainSections {...props} />;
}

function PlainSections({ sections, selected, groupBy, onToggle, onMenu }: Props) {
  return (
    <>
      {sections.map((sec) => (
        <div key={sec.key} className="card">
          <SectionHead sec={sec} selected={selected} onToggle={onToggle} />
          {sec.items.map((it) => (
            <ItemRow key={it.group} it={it} selected={selected} groupBy={groupBy} onToggle={onToggle} onMenu={onMenu} />
          ))}
        </div>
      ))}
    </>
  );
}

type Row =
  { key: string; kind: "head"; sec: Section } | { key: string; kind: "item"; it: Item; last: boolean } | { key: string; kind: "gap" };

function VirtualSections({ sections, selected, groupBy, onToggle, onMenu }: Props) {
  // 摊平成一列：组标题、文件、组间空隙
  const rows = useMemo(() => {
    const out: Row[] = [];
    sections.forEach((sec, i) => {
      if (i > 0) out.push({ key: `gap:${sec.key}`, kind: "gap" });
      out.push({ key: `head:${sec.key}`, kind: "head", sec });
      sec.items.forEach((it, j) => out.push({ key: `item:${it.group}`, kind: "item", it, last: j === sec.items.length - 1 }));
    });
    return out;
  }, [sections]);

  // 滚动的是外层的 .content，列表上方还有文件夹面板等内容
  const listRef = useRef<HTMLDivElement>(null);
  const [scrollEl, setScrollEl] = useState<HTMLElement | null>(null);
  const [margin, setMargin] = useState(0);
  useLayoutEffect(() => {
    const el = listRef.current?.closest<HTMLElement>(".content") ?? null;
    setScrollEl(el);
    if (el && listRef.current) {
      setMargin(listRef.current.getBoundingClientRect().top - el.getBoundingClientRect().top + el.scrollTop);
    }
  });

  const v = useVirtualizer({
    count: rows.length,
    getScrollElement: () => scrollEl,
    estimateSize: (i) => (rows[i].kind === "head" ? 46 : rows[i].kind === "gap" ? 14 : 58),
    getItemKey: (i) => rows[i].key,
    overscan: 12,
    scrollMargin: margin,
  });

  return (
    <div ref={listRef} className="vlist" style={{ height: v.getTotalSize() }}>
      {v.getVirtualItems().map((vi) => {
        const r = rows[vi.index];
        return (
          <div
            key={vi.key}
            data-index={vi.index}
            ref={v.measureElement}
            className={`vrow ${r.kind} ${r.kind === "item" && r.last ? "last" : ""}`}
            style={{ transform: `translateY(${vi.start - margin}px)` }}
          >
            {r.kind === "head" && <SectionHead sec={r.sec} selected={selected} onToggle={onToggle} />}
            {r.kind === "item" && <ItemRow it={r.it} selected={selected} groupBy={groupBy} onToggle={onToggle} onMenu={onMenu} />}
          </div>
        );
      })}
    </div>
  );
}

function SectionHead({ sec, selected, onToggle }: { sec: Section; selected: Set<number>; onToggle: Props["onToggle"] }) {
  const groups = sec.items.map((i) => i.group);
  const on = groups.filter((g) => selected.has(g)).length;
  return (
    <div className="group-head">
      <Checkbox checked={on === groups.length} indeterminate={on > 0} onChange={(v) => onToggle(groups, v)} label={sec.title} />
      {sec.trash ? <Trash size={16} color="var(--danger)" /> : <Folder size={16} color="var(--muted)" />}
      <span className="title">{sec.title}</span>
      {sec.sub && (
        <span className="muted ellipsis mono" style={{ flex: 1 }}>
          {sec.sub}
        </span>
      )}
      {!sec.sub && <span className="spacer" />}
      <span className="muted">{t("{0} 个", [sec.items.length])}</span>
    </div>
  );
}

function ItemRow({
  it,
  selected,
  groupBy,
  onToggle,
  onMenu,
}: {
  it: Item;
  selected: Set<number>;
  groupBy: Props["groupBy"];
  onToggle: Props["onToggle"];
  onMenu: Props["onMenu"];
}) {
  return (
    <div
      className={`list-row clickable ${selected.has(it.group) ? "" : "off"}`}
      onClick={() => onToggle([it.group], !selected.has(it.group))}
    >
      <Checkbox checked={selected.has(it.group)} onChange={(v) => onToggle([it.group], v)} label={it.name} />
      <FileIcon category={it.category} />
      <div className="row-main">
        <div className="row-title ellipsis">
          {it.name}
          {it.renamedTo && (
            <span className="muted" style={{ fontWeight: 400 }}>
              {"  "}
              {t("→ 改名为 {0}", [it.renamedTo ?? ""])}
            </span>
          )}
        </div>
        <div className="item-dst">
          <span className={`chip ${it.manual ? "accent" : ""}`}>{it.reason}</span>
          {groupBy === "rule" && it.kind !== "trash" && (
            <>
              <ArrowRight size={13} />
              <span className="ellipsis">{it.dirDisplay}</span>
            </>
          )}
          {groupBy === "target" && <span className="ellipsis">{t("来自 {0}", [it.originDir])}</span>}
          {it.note && <span className="chip warn">{it.note}</span>}
        </div>
      </div>
      <button
        className="icon-btn"
        aria-label={t("更多")}
        onClick={(e) => {
          e.stopPropagation();
          onMenu(e.currentTarget, it);
        }}
      >
        <Ellipsis size={17} />
      </button>
    </div>
  );
}
