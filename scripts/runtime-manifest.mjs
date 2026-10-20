import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import {
  lstat,
  readFile,
  readdir,
  realpath,
  writeFile,
} from "node:fs/promises";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
const runtime = path.join(root, "runtime");
const manifestPath = path.join(runtime, "runtime-manifest.json");
const normalize = (value) => value.split(path.sep).join("/");
const readJson = async (file) =>
  JSON.parse((await readFile(file, "utf8")).replace(/^\uFEFF/, ""));
const hash = async (file) => {
  const digest = createHash("sha256");
  for await (const chunk of createReadStream(file)) digest.update(chunk);
  return digest.digest("hex");
};
const inside = (base, target) => {
  const relative = path.relative(base, target);
  return (
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
};
async function inspect(directory, entries = []) {
  for (const name of (await readdir(directory)).sort()) {
    const file = path.join(directory, name);
    const stat = await lstat(file);
    if (stat.isSymbolicLink()) {
      // Never package a link that points outside the runtime.
      const resolved = await realpath(file);
      if (!inside(runtime, resolved))
        throw new Error(
          `External runtime symlink: ${normalize(path.relative(runtime, file))}`,
        );
      entries.push({
        path: normalize(path.relative(runtime, file)),
        symlinkTarget: normalize(path.relative(runtime, resolved)),
      });
    } else if (stat.isDirectory()) await inspect(file, entries);
    else if (stat.isFile())
      entries.push({
        path: normalize(path.relative(runtime, file)),
        bytes: stat.size,
        sha256: await hash(file),
      });
    else throw new Error("Unsupported runtime file type");
  }
  return entries;
}
const browser = await readJson(path.join(runtime, "browser-manifest.json"));
if (process.platform !== "win32" || browser.platform !== "windows")
  throw new Error("Stage the Windows runtime on a Windows build host");
const platform = "windows";
const executable = "chrome/chrome.exe";
const browserPath = path.resolve(runtime, executable);
if (
  !inside(runtime, browserPath) ||
  (await hash(browserPath)).toLowerCase() !== browser.sha256.toLowerCase()
)
  throw new Error("Managed browser hash does not match its staging provenance");
const entries = [...(await inspect(path.join(runtime, "chrome")))];
entries.push({
  path: "browser-manifest.json",
  bytes: (await lstat(path.join(runtime, "browser-manifest.json"))).size,
  sha256: await hash(path.join(runtime, "browser-manifest.json")),
});
entries.sort((a, b) => a.path.localeCompare(b.path));
if (process.argv.includes("--write")) {
  await writeFile(
    manifestPath,
    JSON.stringify(
      {
        version: 1,
        platform,
        architecture: process.arch,
        browserVersion: browser.version,
        files: entries,
      },
      null,
      2,
    ) + "\n",
  );
} else {
  const manifest = await readJson(manifestPath);
  if (
    manifest.version !== 1 ||
    manifest.platform !== platform ||
    manifest.architecture !== process.arch ||
    JSON.stringify(manifest.files) !== JSON.stringify(entries)
  )
    throw new Error(
      "Runtime changed since review. Stage reviewed assets and explicitly regenerate the manifest",
    );
}
console.log(
  JSON.stringify({
    runtime: "verified",
    platform,
    browserVersion: browser.version,
    files: entries.length,
  }),
);
