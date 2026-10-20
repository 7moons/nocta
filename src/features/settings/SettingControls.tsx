import {
  createContext,
  useContext,
  useEffect,
  useState,
  type ReactNode,
} from "react";
import { ChevronDown } from "lucide-react";
import { useStableCallback } from "@/hooks/useStableCallback";
import { useUiPreference } from "@/hooks/useUiPreference";

// Active settings search. Rows that don't match hide themselves; a section whose
// own title matches shows all of its rows. Empty outside the settings view.
const SettingsSearch = createContext("");
export const SettingsSearchProvider = SettingsSearch.Provider;

export function matchesSearch(query: string, ...texts: string[]) {
  const haystack = texts.join(" ").toLowerCase();
  return query
    .toLowerCase()
    .split(/\s+/)
    .every((word) => haystack.includes(word));
}

/** DOM id of a settings section, for jumping to it from the category list. */
export function sectionAnchor(id: string) {
  return `settings-${id}`;
}

/**
 * A titled group of settings. `id` is stable across languages: it anchors the section and
 * keys its folded state.
 */
export function SettingSection({
  id,
  title,
  description,
  children,
  collapsible = false,
}: {
  id: string;
  title: string;
  description: string;
  children: ReactNode;
  collapsible?: boolean;
}) {
  const [open, setOpen] = useUiPreference(`section.${id}`, true);
  const query = useContext(SettingsSearch);
  const matched = !query || matchesSearch(query, title, description);
  const body = (
    <SettingsSearch.Provider value={matched ? "" : query}>
      <div className="settings-body">{children}</div>
    </SettingsSearch.Provider>
  );
  if (collapsible)
    return (
      <details
        id={sectionAnchor(id)}
        className="settings-section collapsible-section"
        data-match={matched}
        open={open || Boolean(query)}
        onToggle={(event) => {
          if (!query) setOpen(event.currentTarget.open);
        }}
      >
        <summary>
          <div>
            <h3>{title}</h3>
            <p>{description}</p>
          </div>
          <ChevronDown size={14} />
        </summary>
        {body}
      </details>
    );
  return (
    <section
      id={sectionAnchor(id)}
      className="settings-section"
      data-match={matched}
    >
      <header>
        <h3>{title}</h3>
        <p>{description}</p>
      </header>
      {body}
    </section>
  );
}
export function SettingRow({
  title,
  description,
  children,
  disabled,
}: {
  title: string;
  description: string;
  children: ReactNode;
  disabled?: boolean;
}) {
  const query = useContext(SettingsSearch);
  if (query && !matchesSearch(query, title, description)) return null;
  return (
    <div className={`setting-row ${disabled ? "setting-disabled" : ""}`}>
      <div>
        <h4>{title}</h4>
        <p>{description}</p>
      </div>
      <div className="setting-control">{children}</div>
    </div>
  );
}
export function NumberSetting({
  value,
  onCommit,
  label,
  min = 0,
  max = 3600,
  unit,
}: {
  value: number;
  onCommit: (value: number) => void;
  label: string;
  min?: number;
  max?: number;
  unit?: string;
}) {
  // The typed text while the field has focus; otherwise the saved value shows.
  const [draft, setDraft] = useState<string | null>(null);
  return (
    <div className="number-field">
      <input
        aria-label={label}
        type="number"
        min={min}
        max={max}
        value={draft ?? String(value)}
        onFocus={() => setDraft(String(value))}
        onChange={(event) => setDraft(event.target.value)}
        onBlur={() => {
          const next = Number(draft);
          if (
            draft &&
            Number.isInteger(next) &&
            next >= min &&
            next <= max &&
            next !== value
          )
            onCommit(next);
          setDraft(null);
        }}
      />
      {unit && <span>{unit}</span>}
    </div>
  );
}
export function TextSetting({
  value,
  label,
  placeholder,
  onCommit,
  debounceMs,
  canAutosave,
}: {
  value: string;
  label: string;
  placeholder?: string;
  onCommit: (value: string) => void;
  debounceMs?: number;
  canAutosave?: (value: string) => boolean;
}) {
  // The typed text while the field has focus; otherwise the saved value shows.
  const [draft, setDraft] = useState<string | null>(null);
  const commit = useStableCallback(onCommit);
  const validate = useStableCallback(
    (text: string) => canAutosave?.(text) !== false,
  );
  // Optionally saves while typing, after a pause.
  useEffect(() => {
    if (draft === null || !debounceMs || draft === value || !validate(draft))
      return;
    const timeout = setTimeout(() => commit(draft), debounceMs);
    return () => clearTimeout(timeout);
  }, [draft, value, debounceMs, commit, validate]);
  return (
    <input
      aria-label={label}
      value={draft ?? value}
      placeholder={placeholder}
      onFocus={() => setDraft(value)}
      onChange={(event) => setDraft(event.target.value)}
      onBlur={() => {
        if (draft !== null && draft !== value) onCommit(draft);
        setDraft(null);
      }}
    />
  );
}
