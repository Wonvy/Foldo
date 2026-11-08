// 一个不会被移动的项目，以及它的原因。
import { Ellipsis } from "lucide-react";
import type { SkipView } from "../../api";
import { t } from "../../i18n";
import { FileIcon } from "../ui";

export default function SkipRow({ s, onMenu }: { s: SkipView; onMenu: (el: HTMLElement) => void }) {
  return (
    <div className="list-row">
      <FileIcon category={null} size={28} />
      <div className="row-main">
        <div className="row-title ellipsis">{s.name}</div>
        <div className="row-sub ellipsis">{s.reason}</div>
      </div>
      <button className="icon-btn" aria-label={t("更多")} onClick={(e) => onMenu(e.currentTarget)}>
        <Ellipsis size={17} />
      </button>
    </div>
  );
}
