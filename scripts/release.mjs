import { spawnSync } from "node:child_process";
import { cp, mkdir, readFile, writeFile } from "node:fs/promises";
import { createReadStream } from "node:fs";
import { createHash } from "node:crypto";
import path from "node:path";
import { fileURLToPath } from "node:url";

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), "..");
if (process.platform !== "win32")
  throw new Error("Releases are built on Windows");
const debug = process.argv.includes("--debug");
const bun = "bun.exe";
const run = (executable, args) => {
  const result = spawnSync(executable, args, {
    cwd: root,
    stdio: "inherit",
    shell: false,
  });
  if (result.error) throw result.error;
  if (result.status !== 0)
    throw new Error(`Release step failed with exit status ${result.status}`);
};
run(process.execPath, [path.join(root, "scripts/runtime-manifest.mjs")]);
run(bun, ["run", "check"]);
const config = JSON.parse(
  await readFile(path.join(root, "src-tauri/tauri.runtime.conf.json"), "utf8"),
);
await mkdir(path.join(root, "release"), { recursive: true });
const configPath = path.join(root, "release/build-config.json");
await writeFile(configPath, JSON.stringify(config, null, 2));
run(bun, [
  "run",
  "desktop:build",
  "--config",
  configPath,
  ...(debug ? ["--debug"] : []),
]);
const version = JSON.parse(
  await readFile(path.join(root, "package.json"), "utf8"),
).version;
const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const output = path.join(
  root,
  "release",
  `${version}-${process.platform}-${process.arch}${debug ? "-debug" : ""}-${stamp}`,
);
await mkdir(output);
const built = path.join(root, "target", debug ? "debug" : "release");
// The bundle folder keeps installers from earlier builds; take only this version's.
const installer = `Nocta_${version}_x64-setup.exe`;
await mkdir(path.join(output, "installer"));
await cp(
  path.join(built, "bundle/nsis", installer),
  path.join(output, "installer", installer),
);
const portable = path.join(output, "Nocta-portable");
await mkdir(portable);
await cp(path.join(built, "Nocta.exe"), path.join(portable, "Nocta.exe"));
for (const resource of [
  "chrome",
  "browser-manifest.json",
  "runtime-manifest.json",
])
  await cp(
    path.join(root, "runtime", resource),
    path.join(portable, "runtime", resource),
    { recursive: true, verbatimSymlinks: true },
  );
await writeFile(path.join(portable, "portable.flag"), "");
run(process.execPath, [
  path.join(root, "scripts/verify-package.mjs"),
  portable,
]);
await writeFile(
  path.join(portable, "README.txt"),
  "Run Nocta.exe. This package starts with an empty workspace.\r\nData is created beside Nocta.exe; its key stays in your OS credential store.\r\nTo transfer accounts between computers use a password-protected Nocta backup.\r\nManual updates: exit Nocta, keep the data folder, replace executable and runtime.\r\n",
);
run("powershell.exe", [
  "-NoProfile",
  "-File",
  path.join(root, "scripts/zip-portable.ps1"),
  "-Directory",
  portable,
  "-Destination",
  path.join(
    output,
    `Nocta-${version}-windows-${process.arch}${debug ? "-debug" : ""}-portable.zip`,
  ),
]);
// Hash only what is distributed: the installer and the portable archive, not the unpacked folder.
const { readdir } = await import("node:fs/promises");
const checksums = [];
async function checksumFiles(directory) {
  for (const entry of await readdir(directory, { withFileTypes: true })) {
    const file = path.join(directory, entry.name);
    if (entry.isDirectory() && entry.name !== "Nocta-portable")
      await checksumFiles(file);
    else if (entry.isFile() && /\.(exe|zip)$/.test(entry.name)) {
      const hash = createHash("sha256");
      for await (const chunk of createReadStream(file)) hash.update(chunk);
      // Bare names: the files are downloaded side by side, so `sha256sum -c` works there.
      checksums.push(`${hash.digest("hex")}  ${entry.name}`);
    }
  }
}
await checksumFiles(output);
await writeFile(
  path.join(output, "SHA256SUMS.txt"),
  checksums.join("\n") + "\n",
);
console.log(`Release artifacts: ${output}`);
