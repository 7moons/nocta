import { useEffect, useRef, useState } from "react";

/**
 * A name typed in place: Enter or leaving the field saves it, Escape cancels. An empty or
 * unchanged name cancels too.
 */
export function InlineName({
  initial = "",
  label,
  placeholder,
  onSubmit,
  onCancel,
}: {
  initial?: string;
  label: string;
  placeholder?: string;
  onSubmit: (name: string) => void;
  onCancel: () => void;
}) {
  const [value, setValue] = useState(initial);
  const input = useRef<HTMLInputElement>(null);
  // Enter and the blur that follows must not both submit.
  const finished = useRef(false);
  useEffect(() => {
    input.current?.focus();
    input.current?.select();
  }, []);
  const finish = (save: boolean) => {
    if (finished.current) return;
    finished.current = true;
    const name = value.trim();
    if (save && name && name !== initial) onSubmit(name);
    else onCancel();
  };
  return (
    <input
      ref={input}
      className="inline-name"
      aria-label={label}
      placeholder={placeholder}
      value={value}
      maxLength={80}
      onChange={(event) => setValue(event.target.value)}
      onBlur={() => finish(true)}
      onKeyDown={(event) => {
        event.stopPropagation();
        if (event.key === "Enter") finish(true);
        if (event.key === "Escape") finish(false);
      }}
      onClick={(event) => event.stopPropagation()}
    />
  );
}
