import { useState } from "react";

// Window layout is local to this profile. Storage failures leave the current session usable.
export function useUiPreference<T>(key: string, initial: T) {
  const [value, setValue] = useState<T>(() => {
    try {
      const stored = localStorage.getItem(`nocta.ui.${key}`);
      if (stored === null) return initial;
      const parsed: unknown = JSON.parse(stored);
      if (Array.isArray(initial))
        return Array.isArray(parsed) &&
          parsed.every((item) => typeof item === "string")
          ? (parsed as T)
          : initial;
      return typeof parsed === typeof initial ? (parsed as T) : initial;
    } catch {
      return initial;
    }
  });
  const save = (next: T) => {
    setValue(next);
    try {
      localStorage.setItem(`nocta.ui.${key}`, JSON.stringify(next));
    } catch {
      /* session only */
    }
  };
  return [value, save] as const;
}
