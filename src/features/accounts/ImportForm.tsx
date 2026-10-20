import { useMemo, useRef, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  ClipboardPaste,
  FileUp,
  LoaderCircle,
  Trash2,
  UserPlus,
  UserRoundPen,
} from "lucide-react";
import type {
  Account,
  Folder,
  ImportProgress,
  ImportReport,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { api } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";
import {
  previewImport,
  withoutProblemLines,
  type LineStatus,
} from "./importPreview";

const MAX_FILE_BYTES = 64 * 1024 * 1024;
// The gutter and the problem list stop growing here; the import itself has no such limit.
const MAX_GUTTER_LINES = 5000;
const MAX_LISTED_PROBLEMS = 200;
const COUNTED: Exclude<LineStatus, "ready">[] = [
  "exists",
  "missing",
  "duplicate",
  "invalid",
];

/** The lines to import, checked as they are typed, and the import itself. */
export function ImportForm({
  mode,
  folders,
  folderId,
  accounts,
  busy,
  onBusy,
  onDone,
  onCancel,
}: {
  mode: "add" | "update";
  folders: Folder[];
  folderId: number | null;
  accounts: Account[];
  busy: boolean;
  onBusy: (busy: boolean) => void;
  /** Called with the report and the skipped lines, ready to copy and fix. */
  onDone: (report: ImportReport, skippedLines: string) => void;
  onCancel: () => void;
}) {
  const t = useT();
  const updateExisting = mode === "update";
  const [text, setText] = useState("");
  const [target, setTarget] = useState(folderId?.toString() ?? "");
  const [error, setError] = useState("");
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const fileInput = useRef<HTMLInputElement>(null);
  const gutter = useRef<HTMLDivElement>(null);

  const existing = useMemo(
    () => new Set(accounts.map((account) => account.username.toLowerCase())),
    [accounts],
  );
  const preview = useMemo(
    () => previewImport(text, existing, updateExisting),
    [text, existing, updateExisting],
  );
  const lineCount = Math.max(text.split(/\r?\n/).length, 1);
  const problemLines = useMemo(
    () => new Set(preview.problems.map((line) => line.line)),
    [preview],
  );
  const count = (status: LineStatus) =>
    preview.problems.filter((line) => line.status === status).length;

  async function submit() {
    onBusy(true);
    setError("");
    setProgress(null);
    try {
      const report = await api.importAccounts(
        text,
        target ? Number(target) : null,
        updateExisting,
        setProgress,
      );
      const lines = text.split(/\r?\n/);
      onDone(
        report,
        report.issues.map((issue) => lines[issue.line - 1]).join("\n"),
      );
    } catch (failure) {
      setError(errorMessage(failure));
    } finally {
      onBusy(false);
    }
  }
  async function loadFile(file: File) {
    setError("");
    if (file.size > MAX_FILE_BYTES) {
      setError(t("import.fileTooLarge"));
      return;
    }
    const content = await file.text();
    setText((current) =>
      current.trim() ? `${current.replace(/\s+$/, "")}\n${content}` : content,
    );
  }

  const Icon = updateExisting ? UserRoundPen : UserPlus;
  return (
    <div className="dialog-body import-form">
      <div className="import-format">
        <Icon size={16} />
        <div>
          <strong>{t("import.format.title")}</strong>
          <code>username:password:cookie</code>
          <small>
            {t("import.format.detail", { example: "username::cookie" })}
          </small>
        </div>
      </div>

      <div className="import-options">
        {!updateExisting && (
          <label className="field-label">
            {t("import.folder")}
            <select
              value={target}
              onChange={(event) => setTarget(event.target.value)}
              disabled={busy}
            >
              <option value="">{t("accounts.folders.general")}</option>
              {folders.map((folder) => (
                <option key={folder.id} value={folder.id}>
                  {folder.name}
                </option>
              ))}
            </select>
          </label>
        )}
        <input
          ref={fileInput}
          type="file"
          accept=".txt,text/plain"
          hidden
          onChange={(event) => {
            const file = event.target.files?.[0];
            if (file) void loadFile(file);
            event.target.value = "";
          }}
        />
        <Button
          variant="outline"
          disabled={busy}
          onClick={() => fileInput.current?.click()}
        >
          <FileUp />
          {t("import.loadFile")}
        </Button>
      </div>

      <div className="import-editor">
        <div className="import-gutter" ref={gutter} aria-hidden>
          {Array.from(
            { length: Math.min(lineCount, MAX_GUTTER_LINES) },
            (_, index) => (
              <span
                key={index}
                className={problemLines.has(index + 1) ? "problem" : ""}
              >
                {index + 1}
              </span>
            ),
          )}
        </div>
        <textarea
          autoFocus
          aria-label={t("import.textLabel")}
          spellCheck={false}
          wrap="off"
          value={text}
          disabled={busy}
          onChange={(event) => setText(event.target.value)}
          onScroll={(event) => {
            if (gutter.current)
              gutter.current.scrollTop = event.currentTarget.scrollTop;
          }}
          placeholder={
            "PlayerOne:password:_|WARNING:-DO-NOT-SHARE-THIS...\nPlayerTwo::_|WARNING:-DO-NOT-SHARE-THIS..."
          }
        />
      </div>

      {preview.lines.length > 0 && (
        <div className="import-check" role="status">
          <span className="ready">
            <CheckCircle2 size={13} />
            {t(`import.${mode}.ready`, { count: preview.ready })}
          </span>
          {COUNTED.map(
            (status) =>
              count(status) > 0 && (
                <span
                  key={status}
                  className={status === "invalid" ? "bad" : "warn"}
                >
                  {t(`import.check.${status}`, { count: count(status) })}
                </span>
              ),
          )}
          {preview.problems.length > 0 && (
            <Button
              variant="ghost"
              size="sm"
              className="ml-auto"
              disabled={busy}
              onClick={() =>
                setText(withoutProblemLines(text, preview.problems))
              }
            >
              <Trash2 />
              {t("import.removeProblems")}
            </Button>
          )}
        </div>
      )}
      {preview.problems.length > 0 && (
        <div
          className="import-problems"
          role="list"
          aria-label={t("import.problemsLabel")}
        >
          {preview.problems.slice(0, MAX_LISTED_PROBLEMS).map((line) => (
            <div key={line.line} role="listitem">
              <AlertTriangle size={12} />
              <span className="import-line">
                {t("import.line", { line: line.line })}
              </span>
              <span className="import-user">
                {line.username ?? t("import.noUsername")}
              </span>
              {line.reason && <span>{t(`import.reason.${line.reason}`)}</span>}
            </div>
          ))}
          {preview.problems.length > MAX_LISTED_PROBLEMS && (
            <div>
              {t("import.moreProblems", {
                count: preview.problems.length - MAX_LISTED_PROBLEMS,
              })}
            </div>
          )}
        </div>
      )}

      {error && (
        <div className="error-banner" role="alert">
          {error}
        </div>
      )}
      {busy && progress && (
        <div className="import-progress" role="status">
          <div>
            <span>
              {t(`import.${mode}.progress`, {
                done: progress.processed,
                total: progress.total,
              })}
            </span>
            <span>
              {t("import.progressCounts", {
                added: progress.added,
                updated: progress.updated,
                skipped: progress.failed,
              })}
            </span>
          </div>
          <span className="import-progress-bar">
            <span
              style={{
                width: `${(progress.processed / Math.max(1, progress.total)) * 100}%`,
              }}
            />
          </span>
        </div>
      )}
      <div className="dialog-footer">
        <Button variant="outline" onClick={onCancel} disabled={busy}>
          {t("common.cancel")}
        </Button>
        <Button
          onClick={() => void submit()}
          disabled={busy || preview.ready === 0}
        >
          {busy ? <LoaderCircle className="spin" /> : <ClipboardPaste />}
          {busy
            ? t(`import.${mode}.running`)
            : t(`import.${mode}.submit`, { count: preview.ready })}
        </Button>
      </div>
    </div>
  );
}
