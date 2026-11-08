import { FolderOpen } from "lucide-react";
import { useRef, useState } from "react";
import { errorText } from "../api";
import { pickFolder } from "../store";
import { t } from "../i18n";
import VarMenu from "./VarMenu";

export default function FolderConditionInput({ value, onChange }: { value: string; onChange: (value: string) => void }) {
  const input = useRef<HTMLInputElement>(null);
  const [error, setError] = useState<string | null>(null);
  const choose = (value: string) => {
    setError(null);
    onChange(value);
    input.current?.focus();
  };
  return (
    <div className="grow folder-condition">
      <div className="dest">
        <input
          ref={input}
          className="dest-input mono"
          aria-label={t("所在文件夹")}
          placeholder={t("选择文件夹或位置变量")}
          value={value}
          onChange={(event) => choose(event.target.value)}
        />
        <button
          type="button"
          className="icon-btn"
          title={t("选择文件夹")}
          onClick={async () => {
            try {
              const path = await pickFolder(t("选择文件夹"));
              if (path) choose(path);
            } catch (e) {
              setError(errorText(e));
            }
          }}
        >
          <FolderOpen size={16} />
        </button>
        <VarMenu foldersOnly onInsert={choose} />
      </div>
      {error && (
        <div className="banner error" role="alert">
          {error}
        </div>
      )}
    </div>
  );
}
