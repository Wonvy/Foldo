// 自动整理模式：手动 / 提醒 / 自动。放在整理页顶部。
import { Bell, Hand, Zap } from "lucide-react";
import { useState } from "react";
import { errorText, type AutoMode as Mode, type WatchStatus } from "../api";
import { useStore } from "../store";
import { Segmented } from "./ui";

const HINT: Record<Mode, string> = {
  off: "只在你点“扫描”时整理",
  notify: "后台监听文件夹，有可以整理的文件时发通知，由你确认",
  run: "后台监听文件夹并直接整理，可在历史里撤销；移到回收站仍需你确认",
};

export default function AutoMode({ watch }: { watch: WatchStatus | null }) {
  const { config, save } = useStore();
  const mode: Mode = config.settings?.auto ?? "off";
  const [error, setError] = useState<string | null>(null);

  const status = (() => {
    if (error) return error;
    if (mode === "off" || !watch) return HINT[mode];
    if (watch.error) return watch.error;
    const parts = [HINT[mode]];
    if (watch.last_check) parts.push(`${watch.last_check} 检查过`);
    if (watch.last_done) parts.push(`上次自动整理了 ${watch.last_done} 个文件`);
    return parts.join("\n");
  })();

  return (
    <div className="auto-mode" title={status}>
      <Segmented
        value={mode}
        onChange={async (v) => {
          setError(null);
          try {
            await save({ ...config, settings: { ...config.settings, auto: v === "off" ? undefined : v } });
          } catch (e) {
            setError(errorText(e));
          }
        }}
        options={[
          { value: "off", label: <><Hand size={14} /> 手动</> },
          { value: "notify", label: <><Bell size={14} /> 提醒</> },
          { value: "run", label: <><Zap size={14} /> 自动</> },
        ]}
      />
    </div>
  );
}
