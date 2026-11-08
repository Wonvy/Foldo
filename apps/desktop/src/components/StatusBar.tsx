import { CircleCheck, Circle, TriangleAlert } from "lucide-react";
import { useSaveStatus } from "../saveStatus";
import { useStore } from "../store";
import { t } from "../i18n";
import { Spinner } from "./ui";

export default function StatusBar() {
  const status = useSaveStatus();
  const { config } = useStore();
  return (
    <footer className={`app-status-bar ${status.kind}`} aria-label={t("状态栏")}>
      <div className="app-status-message" role="status" aria-live="polite" aria-atomic="true">
        {status.kind === "saving" ? (
          <Spinner size={13} />
        ) : status.kind === "success" ? (
          <CircleCheck size={13} />
        ) : status.kind === "error" ? (
          <TriangleAlert size={13} />
        ) : (
          <Circle size={11} />
        )}
        <span className="ellipsis" title={status.text}>
          {status.text || t("就绪")}
        </span>
        {status.time > 0 && status.kind !== "saving" && <time>{new Date(status.time).toLocaleTimeString()}</time>}
      </div>
      {status.dirty > 0 && <span className="status-unsaved">{t("有未保存的修改")}</span>}
      <span className="app-status-profile ellipsis" title={config.profile ?? t("默认方案")}>
        {config.profile ?? t("默认方案")}
      </span>
    </footer>
  );
}
