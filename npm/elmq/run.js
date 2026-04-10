#!/usr/bin/env node

const { execFileSync } = require("child_process");
const path = require("path");

const PLATFORMS = {
  "darwin arm64": "@caseywebb/elmq-darwin-arm64",
  "darwin x64": "@caseywebb/elmq-darwin-x64",
  "linux arm64": "@caseywebb/elmq-linux-arm64",
  "linux x64": "@caseywebb/elmq-linux-x64",
};

const key = `${process.platform} ${process.arch}`;
const pkg = PLATFORMS[key];

if (!pkg) {
  console.error(
    `elmq: unsupported platform ${process.platform} ${process.arch}`
  );
  process.exit(1);
}

let bin;
try {
  bin = path.join(require.resolve(`${pkg}/package.json`), "..", "elmq");
} catch {
  console.error(
    `elmq: could not find the platform package ${pkg}. Make sure it was installed — ` +
      `npm may have skipped it if optional dependencies are disabled.`
  );
  process.exit(1);
}

try {
  execFileSync(bin, process.argv.slice(2), { stdio: "inherit" });
} catch (e) {
  process.exit(e.status ?? 1);
}
