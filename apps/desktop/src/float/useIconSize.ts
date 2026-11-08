import { useEffect, useRef, useState } from "react";
import { listen } from "@tauri-apps/api/event";
import { api, errorText } from "../api";

export function useIconSize() {
  const [size, setSize] = useState(64);
  const [error, setError] = useState<string | null>(null);
  const revision = useRef(0);
  useEffect(() => {
    let active = true;
    let unlisten: (() => void) | undefined;
    const initial = revision.current;
    void listen<number>("float-icon-size-changed", (event) => {
      if (active) {
        ++revision.current;
        setSize(event.payload);
      }
    })
      .then((un) => {
        if (active) unlisten = un;
        else un();
      })
      .catch(() => {});
    void api
      .floatIconSize()
      .then((value) => {
        if (active && revision.current === initial) setSize(value);
      })
      .catch(() => {});
    return () => {
      active = false;
      unlisten?.();
    };
  }, []);
  const change = (value: number) => {
    const next = Math.min(112, Math.max(40, Math.round(value)));
    ++revision.current;
    setSize(next);
    setError(null);
    void api.floatIconSize(next).catch((e) => setError(errorText(e)));
  };
  return { size, change, error };
}
