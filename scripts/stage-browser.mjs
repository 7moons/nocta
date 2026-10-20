import { cp, mkdir, stat, readdir, writeFile } from "node:fs/promises";
import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import path from "node:path";
import { fileURLToPath } from "node:url";

// Explicit distribution input; this does not discover or copy a user's installed browser/profile.
const args = process.argv.slice(2);
if (args.length !== 2)
  throw new Error(
    "Usage: bun run runtime:stage-browser <fresh-distribution-directory> <version>",
  );
const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const source = path.resolve(args[0]);
const output = path.join(root, "runtime", "chrome");
if (
  source === output ||
  source.startsWith(`${output}${path.sep}`) ||
  output.startsWith(`${source}${path.sep}`)
)
  throw new Error("Choose a separate fresh distribution directory");
const relative = "chrome.exe";
if (process.platform !== "win32" || !/^\d+\.\d+\.\d+\.\d+$/.test(args[1]))
  throw new Error("Expected Windows and a four-part browser version");
// Fail before any copy if the expected executable is not present.
if (!(await stat(path.join(source, relative))).isFile())
  throw new Error("Expected a managed browser executable");
try {
  if ((await readdir(output)).length)
    throw new Error(
      "Keep the previous runtime separately and stage into an empty runtime/chrome directory",
    );
} catch (error) {
  if (error.code !== "ENOENT") throw error;
}
await mkdir(output, { recursive: true });
await cp(source, output, { recursive: true, verbatimSymlinks: true });
const digest = createHash("sha256");
for await (const chunk of createReadStream(path.join(output, relative)))
  digest.update(chunk);
await writeFile(
  path.join(root, "runtime", "browser-manifest.json"),
  JSON.stringify(
    {
      source: "Explicitly staged Chrome for Testing distribution",
      version: args[1],
      platform: "windows",
      executable: `chrome/${relative}`,
      sha256: digest.digest("hex"),
    },
    null,
    2,
  ) + "\n",
);
console.log(
  "Managed distribution staged. Review the assets, then run bun run runtime:manifest.",
);
