// Mirrors the backend import rules (accounts.rs `import_with_progress`) so the dialog can
// show what will happen before anything is written. Only usernames leave this module;
// passwords and cookies are never part of the result.

export type LineStatus =
  "ready" | "exists" | "missing" | "duplicate" | "invalid";

/** Why a line is skipped; shown through the `import.reason.*` translations. */
export type SkipReason =
  "format" | "noCookie" | "badUsername" | "duplicate" | "exists" | "missing";

export interface PreviewLine {
  line: number;
  /** Username as typed, or null when the line has none. */
  username: string | null;
  status: LineStatus;
  reason: SkipReason | null;
}

export interface ImportPreview {
  lines: PreviewLine[];
  ready: number;
  problems: PreviewLine[];
}

const encoder = new TextEncoder();

function validUsername(name: string) {
  return (
    name.length > 0 &&
    encoder.encode(name).length <= 40 &&
    ![...name].some((char) => char === ":" || /\p{Cc}/u.test(char))
  );
}

export function previewImport(
  text: string,
  existing: Set<string>,
  updateExisting: boolean,
): ImportPreview {
  const lines: PreviewLine[] = [];
  const seen = new Set<string>();
  text.split(/\r?\n/).forEach((raw, index) => {
    const line = raw.trim();
    if (!line) return;
    const number = index + 1;
    const first = line.indexOf(":");
    const second = first < 0 ? -1 : line.indexOf(":", first + 1);
    const username = (first < 0 ? line : line.slice(0, first)).trim();
    const cookie = second < 0 ? "" : line.slice(second + 1).trim();
    const shown = username.slice(0, 40) || null;
    if (second < 0 || !validUsername(username) || !cookie) {
      lines.push({
        line: number,
        username: shown,
        status: "invalid",
        reason: second < 0 ? "format" : !cookie ? "noCookie" : "badUsername",
      });
      return;
    }
    const key = username.toLowerCase();
    if (seen.has(key)) {
      lines.push({
        line: number,
        username,
        status: "duplicate",
        reason: "duplicate",
      });
      return;
    }
    seen.add(key);
    if (!updateExisting && existing.has(key)) {
      lines.push({
        line: number,
        username,
        status: "exists",
        reason: "exists",
      });
      return;
    }
    if (updateExisting && !existing.has(key)) {
      lines.push({
        line: number,
        username,
        status: "missing",
        reason: "missing",
      });
      return;
    }
    lines.push({ line: number, username, status: "ready", reason: null });
  });
  return {
    lines,
    ready: lines.filter((line) => line.status === "ready").length,
    problems: lines.filter((line) => line.status !== "ready"),
  };
}

// Drops the lines that would be skipped, keeping everything else untouched.
export function withoutProblemLines(text: string, problems: PreviewLine[]) {
  const drop = new Set(problems.map((problem) => problem.line));
  return text
    .split(/\r?\n/)
    .filter((_, index) => !drop.has(index + 1))
    .join("\n");
}
