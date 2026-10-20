// Lists translation keys that no code uses: `bun scripts/unused-messages.ts`.
// A key counts as used when its full name appears in the source, or when a template such as
// `settings.engine.${key}` could produce it.
import { readdir, readFile } from "node:fs/promises";
import path from "node:path";
import { en } from "../src/i18n/locales/en";

type Tree = { [key: string]: string | Tree };
const isPlural = (node: unknown) =>
  !!node && typeof node === "object" && "other" in (node as object);

function keys(tree: Tree, prefix = ""): string[] {
  return Object.entries(tree).flatMap(([name, value]) =>
    typeof value === "string" || isPlural(value)
      ? [`${prefix}${name}`]
      : keys(value as Tree, `${prefix}${name}.`),
  );
}

async function sources(directory: string): Promise<string[]> {
  const found: string[] = [];
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory()) {
      if (!["locales", "generated"].includes(entry.name))
        found.push(...(await sources(file)));
    } else if (/\.(ts|tsx)$/.test(entry.name)) found.push(file);
  }
  return found;
}

const text = (
  await Promise.all(
    (await sources(path.resolve("src"))).map((file) => readFile(file, "utf8")),
  )
).join("\n");
// Templates like `settings.engine.${key}Detail` become patterns.
// Only templates with a fixed prefix such as `settings.engine.` count; `${a}${b}` says nothing.
const templates = [
  ...text.matchAll(/`([a-zA-Z][a-zA-Z0-9_]*\.[a-zA-Z0-9_.]*\$\{[^`]*)`/g),
].map(
  (match) =>
    new RegExp(
      "^" +
        match[1]
          .split(/\$\{[^}]*\}/)
          .map((part) => part.replace(/[.*+?^()|[\]\\]/g, "\\$&"))
          .join("[a-zA-Z0-9_.-]+") +
        "$",
    ),
);
const unused = keys(en as unknown as Tree).filter(
  (key) =>
    !text.includes(`"${key}"`) &&
    !text.includes(`'${key}'`) &&
    !text.includes(`\`${key}\``) &&
    !templates.some((pattern) => pattern.test(key)),
);
console.log(unused.join("\n"));
