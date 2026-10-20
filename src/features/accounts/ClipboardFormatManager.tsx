import { useRef, useState, type FormEvent } from "react";
import { Check, Copy, Pencil, Plus, Trash2 } from "lucide-react";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import {
  clipboardFormatStore,
  formatName,
  useClipboardFormats,
  type ClipboardFormat,
} from "./clipboardFormats";
import {
  CLIPBOARD_KEYS,
  isTemplateError,
  parseTemplate,
  previewTemplate,
  templateErrorMessage,
} from "./clipboardTemplate";

const NAME_LIMIT = 40;

/** Lists built-in and custom clipboard formats and edits custom ones, like the theme editor. */
export function ClipboardFormatManager() {
  const t = useT();
  const { formats, selected } = useClipboardFormats();
  const [editing, setEditing] = useState<ClipboardFormat | null>(null);
  if (editing)
    return (
      <ClipboardFormatEditor
        format={editing}
        onClose={() => setEditing(null)}
      />
    );
  const create = (from?: ClipboardFormat) =>
    setEditing({
      id: `custom-${crypto.randomUUID()}`,
      name: from
        ? t("clipboard.copyName", { name: formatName(from, t) }).slice(
            0,
            NAME_LIMIT,
          )
        : t("clipboard.newName"),
      template: from?.template ?? "{username}:{password}",
    });
  return (
    <div className="format-manager">
      <div
        className="format-list"
        role="list"
        aria-label={t("clipboard.title")}
      >
        {formats.map((format) => {
          const name = formatName(format, t);
          const active = format.id === selected.id;
          return (
            <div
              key={format.id}
              role="listitem"
              className={`format-item ${active ? "active" : ""}`}
            >
              <button
                className="format-pick"
                aria-pressed={active}
                onClick={() => clipboardFormatStore.select(format.id)}
              >
                <span className="format-check">
                  {active && <Check size={13} />}
                </span>
                <span className="format-text">
                  <strong>{name}</strong>
                  <code>{format.template}</code>
                </span>
                {format.builtIn && (
                  <span className="badge">{t("clipboard.builtInBadge")}</span>
                )}
              </button>
              <Button
                variant="ghost"
                size="icon"
                title={t("clipboard.duplicate")}
                aria-label={t("clipboard.duplicateNamed", { name })}
                onClick={() => create(format)}
              >
                <Copy />
              </Button>
              {!format.builtIn && (
                <>
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("clipboard.edit")}
                    aria-label={t("clipboard.editNamed", { name })}
                    onClick={() => setEditing(format)}
                  >
                    <Pencil />
                  </Button>
                  <Button
                    variant="ghost"
                    size="icon"
                    title={t("clipboard.delete")}
                    aria-label={t("clipboard.deleteNamed", { name })}
                    onClick={() => {
                      if (
                        window.confirm(t("clipboard.confirmDelete", { name }))
                      )
                        clipboardFormatStore.remove(format.id);
                    }}
                  >
                    <Trash2 />
                  </Button>
                </>
              )}
            </div>
          );
        })}
      </div>
      <Button variant="outline" onClick={() => create()}>
        <Plus />
        {t("clipboard.new")}
      </Button>
    </div>
  );
}

function ClipboardFormatEditor({
  format,
  onClose,
}: {
  format: ClipboardFormat;
  onClose: () => void;
}) {
  const t = useT();
  const [name, setName] = useState(format.name);
  const [template, setTemplate] = useState(format.template);
  const input = useRef<HTMLInputElement>(null);
  const parsed = parseTemplate(template);
  const error = isTemplateError(parsed) ? templateErrorMessage(parsed) : null;
  // Inserts a key at the caret so formats can be built by clicking.
  const insert = (key: string) => {
    const element = input.current;
    const token = `{${key}}`;
    const start = element?.selectionStart ?? template.length;
    const end = element?.selectionEnd ?? template.length;
    setTemplate(template.slice(0, start) + token + template.slice(end));
    requestAnimationFrame(() => {
      element?.focus();
      element?.setSelectionRange(start + token.length, start + token.length);
    });
  };
  function submit(event: FormEvent) {
    event.preventDefault();
    if (error || !name.trim()) return;
    clipboardFormatStore.save({
      id: format.id,
      name: name.trim().slice(0, NAME_LIMIT),
      template,
    });
    onClose();
  }
  return (
    <form className="format-editor" onSubmit={submit}>
      <div className="form-grid format-editor-fields">
        <label className="field-label">
          {t("clipboard.name")}
          <input
            autoFocus
            maxLength={NAME_LIMIT}
            value={name}
            onChange={(event) => setName(event.target.value)}
          />
        </label>
        <label className="field-label">
          {t("clipboard.format")}
          <input
            ref={input}
            className="format-template-input"
            spellCheck={false}
            aria-invalid={!!error}
            value={template}
            onChange={(event) => setTemplate(event.target.value)}
          />
        </label>
      </div>
      <div className="format-keys" aria-label={t("clipboard.insertKey")}>
        {CLIPBOARD_KEYS.map((key) => (
          <button
            type="button"
            key={key}
            className="format-key"
            onClick={() => insert(key)}
          >
            <Plus size={11} />
            {t(`clipboard.key.${key}`)}
          </button>
        ))}
      </div>
      <div className={`format-preview ${error ? "invalid" : ""}`}>
        <span>{t(error ? "clipboard.errorLabel" : "clipboard.preview")}</span>
        <code>{error ?? previewTemplate(template)}</code>
      </div>
      <p className="field-hint">{t("clipboard.hint")}</p>
      <div className="dialog-footer">
        <Button type="button" variant="outline" onClick={onClose}>
          {t("common.cancel")}
        </Button>
        <Button disabled={!!error || !name.trim()}>
          {t("clipboard.save")}
        </Button>
      </div>
    </form>
  );
}
