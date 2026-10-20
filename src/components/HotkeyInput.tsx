import { useState, type KeyboardEvent } from "react";
import { X } from "lucide-react";
import { useT } from "@/i18n";

const KEY_NAMES: Record<string, string> = {
  " ": "Space",
  ArrowUp: "Up",
  ArrowDown: "Down",
  ArrowLeft: "Left",
  ArrowRight: "Right",
};

/** The shortcut for a key press, `Ctrl+Alt+N` style, or null while only modifiers are down. */
export function shortcutFromEvent(event: {
  key: string;
  ctrlKey: boolean;
  altKey: boolean;
  shiftKey: boolean;
  metaKey: boolean;
}): string | null {
  if (["Control", "Alt", "Shift", "Meta"].includes(event.key)) return null;
  const key =
    KEY_NAMES[event.key] ??
    (event.key.length === 1 ? event.key.toUpperCase() : event.key);
  if (
    !/^([A-Z0-9]|F([1-9]|1\d|2[0-4])|Space|Enter|Tab|Home|End|PageUp|PageDown|Insert|Delete|Up|Down|Left|Right)$/.test(
      key,
    )
  )
    return null;
  const parts = [
    event.ctrlKey && "Ctrl",
    event.altKey && "Alt",
    event.shiftKey && "Shift",
    event.metaKey && "Win",
  ].filter(Boolean);
  // Plain keys would fire while typing anywhere; only F-keys may stand alone.
  if (parts.length === 0 && !/^F\d/.test(key)) return null;
  return [...parts, key].join("+");
}

/** Records a shortcut by pressing it. Escape cancels, the clear button removes it. */
export function HotkeyInput({
  value,
  label,
  onChange,
}: {
  value: string;
  label: string;
  onChange: (value: string) => void;
}) {
  const t = useT();
  const [recording, setRecording] = useState(false);
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === "Tab") return;
    event.preventDefault();
    if (event.key === "Escape") {
      event.currentTarget.blur();
      return;
    }
    const shortcut = shortcutFromEvent(event);
    if (shortcut) {
      onChange(shortcut);
      event.currentTarget.blur();
    }
  };
  return (
    <span className="hotkey-input">
      <input
        readOnly
        aria-label={label}
        value={recording ? "" : value}
        placeholder={t(recording ? "hotkeys.recording" : "hotkeys.none")}
        onFocus={() => setRecording(true)}
        onBlur={() => setRecording(false)}
        onKeyDown={onKeyDown}
      />
      {value && (
        <button
          type="button"
          aria-label={t("hotkeys.clear", { name: label })}
          onClick={() => onChange("")}
        >
          <X size={12} />
        </button>
      )}
    </span>
  );
}
