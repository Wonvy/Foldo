// 保持原位的项目：按原因分组（固定保留、凭证文件、文件夹……），默认折叠。
import { ChevronDown, ChevronRight } from "lucide-react";
import type { SkipView } from "../../api";
import { t } from "../../i18n";
import ShowMore from "./ShowMore";
import SkipRow from "./SkipRow";

export default function KeptCard({
  groups,
  open,
  onToggle,
  onMenu,
}: {
  /** [原因, 项目] */
  groups: [string, SkipView[]][];
  open: boolean;
  onToggle: () => void;
  onMenu: (el: HTMLElement, s: SkipView) => void;
}) {
  return (
    <div className="card" id="kept">
      <button
        className="card-head path-link"
        style={{ width: "100%", borderBottom: open ? undefined : 0, textDecoration: "none", color: "inherit" }}
        onClick={onToggle}
      >
        {open ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
        <h2>{t("保持原位的项目")}</h2>
        <span className="muted ellipsis">{groups.map(([label, list]) => `${label} ${list.length}`).join(" · ")}</span>
      </button>
      {open &&
        groups.map(([label, list]) => (
          <div key={label}>
            <div className="section-title" style={{ padding: "12px 16px 4px" }}>
              {label}
              {t("（")}
              {list.length}
              {t("）")}
            </div>
            <ShowMore items={list}>{(s) => <SkipRow key={s.path} s={s} onMenu={(el) => onMenu(el, s)} />}</ShowMore>
          </div>
        ))}
    </div>
  );
}
