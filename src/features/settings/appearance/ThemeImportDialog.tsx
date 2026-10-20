import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/i18n";
import { errorMessage } from "@/lib/utils";
import { themeStore } from "@/theme/store";
import { parseThemeExport } from "@/theme/theme";

/** Adds a theme from text exported by Nocta and applies it. */
export function ThemeImportDialog({
  open,
  onClose,
}: {
  open: boolean;
  onClose: () => void;
}) {
  const t = useT();
  return (
    <Dialog
      open={open}
      onOpenChange={(value) => {
        if (!value) onClose();
      }}
      title={t("theme.importDialog.title")}
      description={t("theme.importDialog.subtitle")}
    >
      {/* Mounted only while open, so every opening starts empty. */}
      {open && <ThemeImportForm onClose={onClose} />}
    </Dialog>
  );
}

function ThemeImportForm({ onClose }: { onClose: () => void }) {
  const t = useT();
  const [text, setText] = useState("");
  const [error, setError] = useState("");
  function submit(event: FormEvent) {
    event.preventDefault();
    try {
      themeStore.save(parseThemeExport(text));
      onClose();
    } catch (failure) {
      setError(errorMessage(failure));
    }
  }
  return (
    <form className="dialog-body" onSubmit={submit}>
      <label className="field-label">
        {t("theme.importDialog.json")}
        <textarea
          autoFocus
          rows={10}
          spellCheck={false}
          value={text}
          onChange={(event) => setText(event.target.value)}
          placeholder='{ "format": "nocta-theme", ... }'
        />
      </label>
      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}
      <div className="dialog-footer">
        <Button variant="outline" type="button" onClick={onClose}>
          {t("common.cancel")}
        </Button>
        <Button disabled={!text.trim()}>
          {t("theme.importDialog.submit")}
        </Button>
      </div>
    </form>
  );
}
