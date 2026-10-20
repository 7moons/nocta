// Clipboard templates are text with account keys, e.g. "{username}:{password}". The backend
// renders them (secrets never reach the page); this module mirrors its parser so mistakes
// show while typing, and builds previews with secrets masked.
import { t } from "@/i18n";

/** Keys a template can use. Labels: `clipboard.key.<key>`. */
export const CLIPBOARD_KEYS = [
  "username",
  "password",
  "cookie",
  "random",
  "userId",
  "id",
  "folder",
] as const;
export type ClipboardKey = (typeof CLIPBOARD_KEYS)[number];

const KEY_ALIASES: Record<string, ClipboardKey> = {
  username: "username",
  user: "username",
  password: "password",
  pass: "password",
  cookie: "cookie",
  random: "random",
  userid: "userId",
  robloxid: "userId",
  id: "id",
  folder: "folder",
};

export const MAX_TEMPLATE_LENGTH = 500;

type Part = { text: string } | { key: ClipboardKey };

export type TemplateError =
  | { error: "length" }
  | { error: "unknownKey"; key: string }
  | { error: "unmatched" };

export function parseTemplate(template: string): Part[] | TemplateError {
  if (!template || template.length > MAX_TEMPLATE_LENGTH)
    return { error: "length" };
  const parts: Part[] = [];
  let text = "";
  for (let index = 0; index < template.length; index++) {
    const char = template[index];
    const next = template[index + 1];
    if ((char === "{" && next === "{") || (char === "}" && next === "}")) {
      text += char;
      index++;
    } else if (char === "{") {
      const end = template.indexOf("}", index);
      const name = template.slice(index + 1, end < 0 ? undefined : end);
      const key = KEY_ALIASES[name.trim().toLowerCase()];
      if (end < 0 || !key) return { error: "unknownKey", key: name };
      if (text) parts.push({ text });
      text = "";
      parts.push({ key });
      index = end;
    } else if (char === "}") {
      return { error: "unmatched" };
    } else text += char;
  }
  if (text) parts.push({ text });
  return parts;
}

export const isTemplateError = (
  parsed: Part[] | TemplateError,
): parsed is TemplateError => !Array.isArray(parsed);

export function templateErrorMessage(error: TemplateError) {
  return error.error === "unknownKey"
    ? t("clipboard.error.unknownKey", { key: error.key })
    : t(`clipboard.error.${error.error}`, { max: MAX_TEMPLATE_LENGTH });
}

export interface SampleAccount {
  username: string;
  userId?: string | null;
  id?: number;
  folder?: string;
}

function sample(key: ClipboardKey, account: SampleAccount | undefined) {
  switch (key) {
    case "username":
      return account?.username ?? "PlayerName";
    case "password":
      return "••••••••";
    case "cookie":
      return "_|WARNING:…";
    case "random":
      return "k7Qm2xRb9LwT4nHs";
    case "userId":
      return account?.userId ?? "123456789";
    case "id":
      return String(account?.id ?? 1);
    case "folder":
      return account?.folder ?? t("accounts.folders.general");
  }
}

/** One line of the template for `account` (or a sample one), with secrets masked. */
export function previewTemplate(template: string, account?: SampleAccount) {
  const parts = parseTemplate(template);
  if (isTemplateError(parts)) return null;
  return parts
    .map((part) => ("text" in part ? part.text : sample(part.key, account)))
    .join("");
}
