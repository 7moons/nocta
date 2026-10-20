import { useState } from "react";
import { CheckCircle2, Copy } from "lucide-react";
import type { Account, Folder, ImportReport } from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { Dialog } from "@/components/ui/dialog";
import { useT } from "@/i18n";
import { errorMessage } from "@/lib/utils";
import { BrowserSignIn } from "./BrowserSignIn";
import { ImportForm } from "./ImportForm";

/** Adds new accounts, or refreshes credentials of existing ones, from pasted lines or a file. */
export function ImportDialog({
  open,
  onClose,
  folders,
  folderId,
  accounts,
  updateExisting = false,
}: {
  open: boolean;
  onClose: () => void;
  folders: Folder[];
  /** Folder new accounts go to by default. */
  folderId: number | null;
  accounts: Account[];
  updateExisting?: boolean;
}) {
  const t = useT();
  const mode = updateExisting ? "update" : "add";
  const [busy, setBusy] = useState(false);
  const [result, setResult] = useState<{
    report: ImportReport;
    skippedLines: string;
  } | null>(null);
  // Each opening and each "Import more" starts a fresh form.
  const [round, setRound] = useState(0);
  const finish = () => {
    setResult(null);
    setRound(round + 1);
    onClose();
  };
  return (
    <Dialog
      open={open}
      wide
      onOpenChange={(value) => {
        if (!value && !busy) finish();
      }}
      title={t(`import.${mode}.title`)}
      description={t(`import.${mode}.subtitle`)}
    >
      {result ? (
        <ImportResult
          mode={mode}
          report={result.report}
          skippedLines={result.skippedLines}
          onMore={() => {
            setResult(null);
            setRound(round + 1);
          }}
          onDone={finish}
        />
      ) : (
        open && (
          <>
            {mode === "add" && (
              <BrowserSignIn
                folderId={folderId}
                busy={busy}
                onBusy={setBusy}
                onDone={finish}
              />
            )}
            <ImportForm
              key={round}
              mode={mode}
              folders={folders}
              folderId={folderId}
              accounts={accounts}
              busy={busy}
              onBusy={setBusy}
              onDone={(report, skippedLines) =>
                setResult({ report, skippedLines })
              }
              onCancel={finish}
            />
          </>
        )
      )}
    </Dialog>
  );
}

function ImportResult({
  mode,
  report,
  skippedLines,
  onMore,
  onDone,
}: {
  mode: "add" | "update";
  report: ImportReport;
  skippedLines: string;
  onMore: () => void;
  onDone: () => void;
}) {
  const t = useT();
  const [copied, setCopied] = useState(false);
  const [error, setError] = useState("");
  return (
    <div className="dialog-body import-result">
      <div className="import-result-head">
        <CheckCircle2 size={22} />
        <div>
          <strong>{t(`import.${mode}.finished`)}</strong>
          <small>
            {report.skipped
              ? t("import.result.skipped", { count: report.skipped })
              : t("import.result.allImported")}
          </small>
        </div>
      </div>
      <div className="import-summary">
        <span className="added">
          <strong>{report.added}</strong> {t("import.result.added")}
        </span>
        <span className="updated">
          <strong>{report.updated}</strong> {t("import.result.updated")}
        </span>
        <span className={report.skipped ? "skipped" : ""}>
          <strong>{report.skipped}</strong> {t("import.result.skippedLabel")}
        </span>
      </div>
      {report.issues.length > 0 && (
        <div className="import-problems" role="list">
          {report.issues.map((issue) => (
            <div key={issue.line} role="listitem">
              <span className="import-line">
                {t("import.line", { line: issue.line })}
              </span>
              <span>{issue.reason}</span>
            </div>
          ))}
        </div>
      )}
      {error && (
        <p className="form-error" role="alert">
          {error}
        </p>
      )}
      <div className="dialog-footer">
        {skippedLines && (
          <Button
            variant="outline"
            onClick={() =>
              void navigator.clipboard
                .writeText(skippedLines)
                .then(() => setCopied(true))
                .catch((failure) => setError(errorMessage(failure)))
            }
          >
            <Copy />
            {t(copied ? "import.result.copied" : "import.result.copySkipped")}
          </Button>
        )}
        <Button variant="outline" onClick={onMore}>
          {t(`import.${mode}.more`)}
        </Button>
        <Button onClick={onDone}>{t("import.result.done")}</Button>
      </div>
    </div>
  );
}
