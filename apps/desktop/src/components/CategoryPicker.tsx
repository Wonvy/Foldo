import { Check, ChevronDown } from "lucide-react";
import { useEffect, useId, useRef, useState } from "react";
import { useStore } from "../store";
import { t } from "../i18n";
import Popover from "./Popover";
import { FileIcon } from "./ui";

/** Native select options cannot reliably display hover details in WebView. */
export default function CategoryPicker({
  options,
  value,
  multiple = false,
  onChange,
}: {
  options: string[];
  value: unknown;
  multiple?: boolean;
  onChange: (value: string | string[]) => void;
}) {
  const { categories, config } = useStore();
  const anchor = useRef<HTMLButtonElement>(null);
  const list = useRef<HTMLDivElement>(null);
  const id = useId();
  const [open, setOpen] = useState(false);
  const [active, setActive] = useState(String(value ?? options[0] ?? ""));
  const extensions = (name: string) => {
    const category = categories.find((c) => c.name === name || c.id === name || c.aliases.includes(name));
    return category ? (config.settings?.category_extensions?.[category.id] ?? category.extensions) : [];
  };
  const description = (name: string) => {
    const suffixes = extensions(name);
    return suffixes.length ? suffixes.map((ext) => `.${ext}`).join("  ") : t("未配置扩展名");
  };
  const close = (restoreFocus = false) => {
    setOpen(false);
    if (restoreFocus) anchor.current?.focus();
  };
  const show = () => {
    setActive(options.includes(String(value)) ? String(value) : (options[0] ?? ""));
    setOpen(true);
  };
  useEffect(() => {
    if (open) list.current?.focus();
  }, [open]);
  useEffect(() => {
    if (open) list.current?.querySelector<HTMLElement>(".active")?.scrollIntoView?.({ block: "nearest" });
  }, [open, active]);

  if (multiple) {
    const selected = new Set(Array.isArray(value) ? (value as string[]) : []);
    return (
      <span className="chip-select grow">
        {options.map((name) => (
          <button
            type="button"
            key={name}
            className={`chip ${selected.has(name) ? "accent" : ""}`}
            title={description(name)}
            onClick={() => {
              const next = new Set(selected);
              if (next.has(name)) next.delete(name);
              else next.add(name);
              onChange([...next]);
            }}
          >
            {t(name)}
          </button>
        ))}
      </span>
    );
  }

  const suffixes = extensions(active);
  return (
    <>
      <button
        type="button"
        ref={anchor}
        className="input grow category-select"
        aria-label={t("文件类型")}
        aria-haspopup="listbox"
        aria-expanded={open}
        aria-controls={open ? id : undefined}
        title={description(String(value ?? ""))}
        onClick={() => (open ? close() : show())}
        onKeyDown={(event) => {
          if (event.key === "ArrowDown" || event.key === "ArrowUp") {
            event.preventDefault();
            show();
          }
        }}
      >
        <FileIcon category={String(value ?? "")} size={16} />
        <span className="ellipsis">{t(String(value ?? ""))}</span>
        <ChevronDown size={14} className="chev" />
      </button>
      {open && (
        <Popover
          anchor={anchor}
          onClose={() => close(document.activeElement === list.current)}
          width={Math.min(520, window.innerWidth - 16)}
        >
          <div className="category-picker">
            <div
              ref={list}
              id={id}
              role="listbox"
              tabIndex={0}
              aria-label={t("文件类型")}
              aria-activedescendant={`${id}-${options.indexOf(active)}`}
              className="category-picker-list"
              onKeyDown={(event) => {
                const index = options.indexOf(active);
                let next = index;
                if (event.key === "ArrowDown") next = Math.min(options.length - 1, index + 1);
                else if (event.key === "ArrowUp") next = Math.max(0, index - 1);
                else if (event.key === "Home") next = 0;
                else if (event.key === "End") next = options.length - 1;
                else if (event.key === "Enter" || event.key === " ") {
                  event.preventDefault();
                  onChange(active);
                  close(true);
                  return;
                } else if (event.key === "Tab") {
                  close(true);
                  return;
                } else return;
                event.preventDefault();
                if (options[next]) setActive(options[next]);
              }}
            >
              {options.map((name, index) => (
                <div
                  key={name}
                  id={`${id}-${index}`}
                  role="option"
                  aria-selected={value === name}
                  aria-describedby={active === name ? `${id}-details` : undefined}
                  className={`category-picker-option ${active === name ? "active" : ""}`}
                  onMouseEnter={() => setActive(name)}
                  onMouseDown={(event) => event.preventDefault()}
                  onClick={() => {
                    onChange(name);
                    close(true);
                  }}
                >
                  <FileIcon category={name} size={16} />
                  <span>{t(name)}</span>
                  {value === name && <Check size={14} className="category-selected" />}
                </div>
              ))}
            </div>
            <div className="category-picker-details" id={`${id}-details`} role="tooltip" aria-live="polite">
              <strong>{t(active)}</strong>
              <span className="muted">{t("{0} 个扩展名", [suffixes.length])}</span>
              <div className="category-picker-extensions">
                {suffixes.length ? (
                  suffixes.map((ext) => <code key={ext}>.{ext}</code>)
                ) : (
                  <span className="muted">{t("未配置扩展名")}</span>
                )}
              </div>
            </div>
          </div>
        </Popover>
      )}
    </>
  );
}
