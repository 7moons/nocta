import { useState, type FormEvent } from "react";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";

type Mode = "export" | "restore";

// Matches the backend's minimum for export passwords.
const MIN_PASSWORD = 12;

/** Exports or restores a password-protected backup; the file is chosen in a native dialog. */
export function BackupDialog({
  mode,
  onClose,
}: {
  mode: Mode | null;
  onClose: () => void;
}) {
  const t = useT();
  const [busy, setBusy] = useState(false);
  return (
    <Dialog
      open={mode !== null}
      onOpenChange={(open) => {
        if (!open && !busy) onClose();
      }}
      title={t(
        mode === "export" ? "backup.exportTitle" : "backup.restoreTitle",
      )}
      description={t(
        mode === "export" ? "backup.exportDetail" : "backup.restoreDetail",
      )}
    >
      {mode && (
        <BackupForm
          key={mode}
          mode={mode}
          busy={busy}
          onBusy={setBusy}
          onClose={onClose}
        />
      )}
    </Dialog>
  );
}

function BackupForm({
  mode,
  busy,
  onBusy,
  onClose,
}: {
  mode: Mode;
  busy: boolean;
  onBusy: (busy: boolean) => void;
  onClose: () => void;
}) {
  const t = useT();
  const [password, setPassword] = useState("");
  const [confirmation, setConfirmation] = useState("");
  const [error, setError] = useState("");
  const [result, setResult] = useState("");
  const exporting = mode === "export";

  async function submit(event: FormEvent) {
    event.preventDefault();
    onBusy(true);
    setError("");
    try {
      // Both resolve to null when the file dialog is cancelled.
      if (exporting) {
        if (await api.exportBackup(password)) setResult(t("backup.exported"));
      } else {
        const count = await api.restoreBackup(password);
        if (count !== null) setResult(t("backup.restored", { count }));
      }
      setPassword("");
      setConfirmation("");
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      onBusy(false);
    }
  }

  const ready =
    !!password &&
    (!exporting ||
      (Array.from(password).length >= MIN_PASSWORD &&
        password === confirmation));
  return (
    <form className="dialog-body" onSubmit={(event) => void submit(event)}>
      {result ? (
        <p role="status">{result}</p>
      ) : (
        <>
          <label className="field-label">
            {t("backup.password")}
            <input
              autoFocus
              type="password"
              autoComplete="new-password"
              value={password}
              onChange={(event) => setPassword(event.target.value)}
              minLength={exporting ? MIN_PASSWORD : 1}
              required
            />
          </label>
          {exporting && (
            <>
              <p>{t("backup.passwordHint", { min: MIN_PASSWORD })}</p>
              <label className="field-label">
                {t("backup.confirm")}
                <input
                  type="password"
                  autoComplete="new-password"
                  value={confirmation}
                  onChange={(event) => setConfirmation(event.target.value)}
                  required
                />
              </label>
            </>
          )}
        </>
      )}
      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}
      <div className="dialog-footer">
        <Button
          variant="outline"
          type="button"
          disabled={busy}
          onClick={onClose}
        >
          {t(result ? "import.result.done" : "common.cancel")}
        </Button>
        {!result && (
          <Button disabled={busy || !ready}>
            {t(busy ? "backup.working" : "backup.chooseFile")}
          </Button>
        )}
      </div>
    </form>
  );
}
