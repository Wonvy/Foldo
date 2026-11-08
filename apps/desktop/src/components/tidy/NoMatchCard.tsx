// 没有匹配规则的文件：按扩展名分组，点开看具体文件，可以直接新建规则。
import { ChevronDown, ChevronRight, Ellipsis, Sparkles, WandSparkles } from "lucide-react";
import type { SkipView } from "../../api";
import { t } from "../../i18n";
import { FileIcon } from "../ui";
import ShowMore from "./ShowMore";

export default function NoMatchCard({
  groups,
  expanded,
  onToggle,
  onNewRule,
  onMenu,
}: {
  /** [扩展名, 文件] */
  groups: [string, SkipView[]][];
  expanded: Set<string>;
  onToggle: (extension: string) => void;
  onNewRule: () => void;
  onMenu: (el: HTMLElement, s: SkipView) => void;
}) {
  return (
    <div className="card" id="no-match">
      <div className="card-head">
        <Sparkles size={16} color="var(--accent)" />
        <h2>{t("没有匹配规则的文件")}</h2>
        <span className="muted">{t("这些文件保持原位")}</span>
        <span className="spacer" />
        <button className="btn sm" onClick={onNewRule}>
          <WandSparkles size={14} /> {t("新建规则")}
        </button>
      </div>
      {groups.map(([extension, files]) => (
        <div key={extension}>
          <button
            type="button"
            className="group-head path-link"
            style={{ width: "100%", textAlign: "left", textDecoration: "none", color: "inherit" }}
            aria-expanded={expanded.has(extension)}
            onClick={() => onToggle(extension)}
          >
            {expanded.has(extension) ? <ChevronDown size={16} /> : <ChevronRight size={16} />}
            <span className="title">{extension || t("无扩展名")}</span>
            <span className="spacer" />
            <span className="muted">{t("{0} 个", [files.length])}</span>
          </button>
          {expanded.has(extension) && (
            <ShowMore items={files}>
              {(s) => (
                <div key={s.path} className="list-row">
                  <FileIcon name={s.name} />
                  <div className="row-main">
                    <div className="row-title ellipsis">{s.name}</div>
                    <div className="row-sub ellipsis">{s.display}</div>
                  </div>
                  <button className="icon-btn" aria-label={t("更多")} onClick={(e) => onMenu(e.currentTarget, s)}>
                    <Ellipsis size={17} />
                  </button>
                </div>
              )}
            </ShowMore>
          )}
        </div>
      ))}
    </div>
  );
}
