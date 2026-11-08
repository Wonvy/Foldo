// 很长的列表先显示前一部分，点“显示全部”再展开，避免一次渲染几千行。
import { useState, type ReactNode } from "react";
import { t } from "../../i18n";

const FIRST = 200;

export default function ShowMore<T>({ items, children }: { items: T[]; children: (item: T) => ReactNode }) {
  const [all, setAll] = useState(false);
  const shown = all ? items : items.slice(0, FIRST);
  return (
    <>
      {shown.map(children)}
      {items.length > shown.length && (
        <div className="list-row">
          <button className="btn sm" onClick={() => setAll(true)}>
            {t("显示全部 {0} 个", [items.length])}
          </button>
        </div>
      )}
    </>
  );
}
