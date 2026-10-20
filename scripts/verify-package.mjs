import { createHash } from "node:crypto";
import { createReadStream } from "node:fs";
import { lstat, readFile, realpath } from "node:fs/promises";
import path from "node:path";
import assert from "node:assert/strict";

// Check the actual packaged files, independently of the project's staged runtime.
const packageRoot = path.resolve(process.argv[2] ?? "");
assert(
  process.argv[2],
  "Usage: node scripts/verify-package.mjs <portable-root>",
);
const runtime = path.join(packageRoot, "runtime");
const manifest = JSON.parse(
  await readFile(path.join(runtime, "runtime-manifest.json"), "utf8"),
);
assert.equal(manifest.version, 1);
const inside = (file) => {
  const relative = path.relative(runtime, file);
  return (
    relative !== ".." &&
    !relative.startsWith(`..${path.sep}`) &&
    !path.isAbsolute(relative)
  );
};
for (const entry of manifest.files) {
  const file = path.resolve(runtime, entry.path);
  assert(inside(file), "Runtime manifest path escapes the package");
  const stat = await lstat(file);
  if (entry.symlinkTarget) {
    assert(stat.isSymbolicLink());
    assert.equal(
      path
        .relative(runtime, await realpath(file))
        .split(path.sep)
        .join("/"),
      entry.symlinkTarget,
    );
  } else {
    assert(stat.isFile());
    assert.equal(stat.size, entry.bytes, `Size mismatch: ${entry.path}`);
    const digest = createHash("sha256");
    for await (const chunk of createReadStream(file)) digest.update(chunk);
    assert.equal(
      digest.digest("hex"),
      entry.sha256,
      `Hash mismatch: ${entry.path}`,
    );
  }
}
assert((await lstat(path.join(packageRoot, "portable.flag"))).isFile());
assert((await lstat(path.join(packageRoot, "Nocta.exe"))).isFile());
console.log(
  JSON.stringify({
    packagedRuntime: "verified",
    files: manifest.files.length,
    platform: manifest.platform,
    architecture: manifest.architecture,
  }),
);
