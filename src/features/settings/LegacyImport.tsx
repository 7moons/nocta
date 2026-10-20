import { useCallback, useEffect, useState } from "react";
import {
  AlertTriangle,
  CheckCircle2,
  Database,
  FolderSearch,
  LoaderCircle,
  RefreshCw,
} from "lucide-react";
import type {
  ImportProgress,
  ImportReport,
  LegacySource,
} from "@/generated/contracts";
import { Button } from "@/components/ui/button";
import { useT } from "@/i18n";
import { useFormat } from "@/i18n/format";
import { api, desktop } from "@/lib/ipc";
import { errorMessage } from "@/lib/utils";

const MAX_LISTED_ISSUES = 300;

/**
 * Finds databases of the old Nocta (and earlier apps) and files of ic3w0lf's Roblox Account
 * Manager, and imports their accounts and folders. The files are only read; they stay as they
 * are.
 */
export function LegacyImport() {
  const t = useT();
  const format = useFormat();
  const [sources, setSources] = useState<LegacySource[] | null>(null);
  const [scanning, setScanning] = useState(false);
  const [includeTrash, setIncludeTrash] = useState(false);
  const [importing, setImporting] = useState<string | null>(null);
  const [progress, setProgress] = useState<ImportProgress | null>(null);
  const [report, setReport] = useState<{
    source: LegacySource;
    result: ImportReport;
  } | null>(null);
  const [error, setError] = useState("");
  // Passwords for files protected by the other app's password, by path.
  const [passwords, setPasswords] = useState<Record<string, string>>({});

  const scan = useCallback(() => {
    // The browser preview cannot read local files.
    if (!desktop) {
      setSources([]);
      return;
    }
    setScanning(true);
    setError("");
    void api
      .legacyDetect()
      .then(setSources)
      .catch((reason) => setError(errorMessage(reason)))
      .finally(() => setScanning(false));
  }, []);
  useEffect(scan, [scan]);

  const pick = () => {
    setError("");
    void api
      .legacyPick()
      .then((source) => {
        if (source)
          setSources((current) => [
            source,
            ...(current ?? []).filter((item) => item.path !== source.path),
          ]);
      })
      .catch((reason) => setError(errorMessage(reason)));
  };

  const start = (source: LegacySource) => {
    const count = includeTrash
      ? source.accounts
      : source.accounts - source.inTrash;
    if (
      !window.confirm(
        source.needsPassword
          ? t("legacy.confirmLocked", { app: source.app })
          : t("legacy.confirm", {
              accounts: count,
              folders: source.folders,
              app: source.app,
            }),
      )
    )
      return;
    setImporting(source.path);
    setProgress(null);
    setReport(null);
    setError("");
    void api
      .legacyImport(
        source.path,
        includeTrash,
        setProgress,
        passwords[source.path] || null,
      )
      .then((result) => setReport({ source, result }))
      .catch((reason) => setError(errorMessage(reason)))
      .finally(() => setImporting(null));
  };

  return (
    <div className="legacy-import">
      {report ? (
        <LegacyResult
          app={report.source.app}
          report={report.result}
          onDone={() => setReport(null)}
        />
      ) : (
        <>
          <div
            className="legacy-sources"
            role="list"
            aria-label={t("legacy.sources")}
          >
            {sources === null || scanning ? (
              <p className="client-note">
                <LoaderCircle className="spin" size={13} />{" "}
                {t("legacy.scanning")}
              </p>
            ) : sources.length === 0 ? (
              <p className="client-note">{t("legacy.none")}</p>
            ) : (
              sources.map((source) => (
                <div
                  key={source.path}
                  role="listitem"
                  className="legacy-source"
                >
                  <span className="client-status-icon ready">
                    <Database size={17} />
                  </span>
                  <div>
                    <strong>{source.app}</strong>
                    <small>
                      {source.needsPassword
                        ? t("legacy.locked")
                        : t("legacy.summary", {
                            accounts: source.accounts,
                            folders: source.folders,
                          })}
                      {source.inTrash > 0 &&
                        ` · ${t("legacy.inTrash", { count: source.inTrash })}`}
                      {source.modifiedAt &&
                        ` · ${t("legacy.lastUsed", { date: format.date(source.modifiedAt) })}`}
                    </small>
                    <code title={source.path}>{source.path}</code>
                    {source.needsPassword && (
                      <input
                        type="password"
                        autoComplete="off"
                        aria-label={t("legacy.password", { app: source.app })}
                        placeholder={t("legacy.passwordPlaceholder")}
                        value={passwords[source.path] ?? ""}
                        onChange={(event) =>
                          setPasswords({
                            ...passwords,
                            [source.path]: event.target.value,
                          })
                        }
                      />
                    )}
                  </div>
                  <Button
                    disabled={
                      importing !== null ||
                      (source.needsPassword && !passwords[source.path])
                    }
                    onClick={() => start(source)}
                  >
                    {importing === source.path && (
                      <LoaderCircle className="spin" />
                    )}
                    {t(
                      importing === source.path
                        ? "import.add.running"
                        : "legacy.import",
                    )}
                  </Button>
                </div>
              ))
            )}
          </div>
          {importing && progress && (
            <div className="import-progress" role="status">
              <div>
                <span>
                  {t("legacy.progress", {
                    done: progress.processed,
                    total: progress.total,
                  })}
                </span>
                <span>
                  {t("legacy.progressCounts", {
                    added: progress.added,
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
          <label className="checkbox-label legacy-option">
            <input
              type="checkbox"
              checked={includeTrash}
              disabled={importing !== null}
              onChange={(event) => setIncludeTrash(event.target.checked)}
            />
            {t("legacy.includeTrash")}
          </label>
          <div className="client-actions">
            <Button
              variant="outline"
              disabled={importing !== null || scanning}
              onClick={scan}
            >
              <RefreshCw />
              {t("legacy.rescan")}
            </Button>
            <Button
              variant="outline"
              disabled={importing !== null || !desktop}
              onClick={pick}
            >
              <FolderSearch />
              {t("legacy.pick")}
            </Button>
          </div>
        </>
      )}
      {error && (
        <div className="error-banner client-error" role="alert">
          <AlertTriangle size={14} />
          <span>{error}</span>
        </div>
      )}
    </div>
  );
}

function LegacyResult({
  app,
  report,
  onDone,
}: {
  app: string;
  report: ImportReport;
  onDone: () => void;
}) {
  const t = useT();
  return (
    <div className="legacy-result" role="status">
      <div className="import-result-head">
        <CheckCircle2 size={22} />
        <div>
          <strong>{t("legacy.done", { app })}</strong>
          <small>
            {t(report.skipped ? "legacy.resultSkipped" : "legacy.result", {
              added: report.added,
              skipped: report.skipped,
            })}
          </small>
        </div>
      </div>
      {report.issues.length > 0 && (
        <div
          className="import-problems"
          role="list"
          aria-label={t("legacy.skippedList")}
        >
          {report.issues.slice(0, MAX_LISTED_ISSUES).map((issue) => (
            <div key={issue.line} role="listitem">
              <AlertTriangle size={12} />
              <span>{issue.reason}</span>
            </div>
          ))}
          {report.issues.length > MAX_LISTED_ISSUES && (
            <div>
              {t("import.moreProblems", {
                count: report.issues.length - MAX_LISTED_ISSUES,
              })}
            </div>
          )}
        </div>
      )}
      <div className="client-actions">
        <Button onClick={onDone}>{t("import.result.done")}</Button>
      </div>
    </div>
  );
}
