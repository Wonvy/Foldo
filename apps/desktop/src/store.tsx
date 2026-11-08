// 全局状态：配置文件与内置分类。各页面通过 useStore() 读取和保存。
import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { api, errorText, type Category, type ConfigView, type FieldDef, type RuleFile, type VarDef } from "./api";
import { pruneFolderRules, setHome, type FieldMap } from "./rules";
import { t } from "./i18n";

type Store = {
  view: ConfigView | null;
  /** 当前配置；文件不存在时为一份空配置。 */
  config: RuleFile;
  categories: Category[];
  /** 规则能用的全部字段，以及字段之外的变量。 */
  fields: FieldDef[];
  fieldMap: FieldMap;
  vars: VarDef[];
  loadError: string | null;
  reload: () => Promise<void>;
  /** 保存整份配置；失败时抛出错误信息。 */
  save: (next: RuleFile) => Promise<void>;
  setView: (v: ConfigView) => void;
};

const EMPTY: RuleFile = { version: 2, folders: [], protect: [], settings: { buffer: "10m" }, rules: [] };

const Ctx = createContext<Store | null>(null);

export function StoreProvider({ children }: { children: ReactNode }) {
  const [view, setView] = useState<ConfigView | null>(null);
  const [categories, setCategories] = useState<Category[]>([]);
  const [fields, setFields] = useState<FieldDef[]>([]);
  const [vars, setVars] = useState<VarDef[]>([]);
  const [loadError, setLoadError] = useState<string | null>(null);

  const reload = useCallback(async () => {
    try {
      const v = await api.getConfig();
      setHome(v.home);
      setView(v);
      setLoadError(null);
    } catch (e) {
      setLoadError(errorText(e));
    }
  }, []);

  useEffect(() => {
    void reload();
    api.categories().then(setCategories).catch(() => setCategories([]));
    api
      .fields()
      .then((f) => {
        // 字段名称和说明来自后端（简体中文），这里换成界面语言；分组和可选值保持原样（它们也用于查找和保存）
        setFields(f.fields.map((x) => ({ ...x, label: t(x.label), desc: t(x.desc), unit: x.unit && t(x.unit) })));
        setVars(f.vars.map((x) => ({ ...x, label: t(x.label), desc: t(x.desc) })));
      })
      .catch(() => setFields([]));
  }, [reload]);

  const save = useCallback(async (next: RuleFile) => {
    try {
      setView(await api.saveConfig(pruneFolderRules(next)));
    } catch (e) {
      throw errorText(e);
    }
  }, []);

  const config = view?.config ?? EMPTY;
  const fieldMap = useMemo(() => new Map(fields.map((f) => [f.key, f])), [fields]);
  return (
    <Ctx.Provider value={{ view, config, categories, fields, fieldMap, vars, loadError, reload, save, setView }}>
      {children}
    </Ctx.Provider>
  );
}

export function useStore(): Store {
  const s = useContext(Ctx);
  if (!s) throw new Error(t("StoreProvider 缺失"));
  return s;
}

/** 选择文件夹（系统对话框）。取消时返回 null。 */
export async function pickFolder(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: true, multiple: false, title });
  return typeof r === "string" ? r : null;
}

/** 选择文件（系统对话框）。取消时返回 null。 */
/** 选择规则文件（YAML）。 */
export async function pickRulesFile(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: false, multiple: false, title, filters: [{ name: t("Foldo 规则"), extensions: ["yaml", "yml"] }] });
  return typeof r === "string" ? r : null;
}

/** 选择保存位置。取消时返回 null。 */
export async function pickSavePath(title: string, defaultPath: string): Promise<string | null> {
  const { save } = await import("@tauri-apps/plugin-dialog");
  const r = await save({ title, defaultPath, filters: [{ name: t("Foldo 规则"), extensions: ["yaml"] }] });
  return r ?? null;
}

export async function pickFile(title: string): Promise<string | null> {
  const { open } = await import("@tauri-apps/plugin-dialog");
  const r = await open({ directory: false, multiple: false, title });
  return typeof r === "string" ? r : null;
}
