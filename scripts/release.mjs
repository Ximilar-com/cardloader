#!/usr/bin/env node
// Releases a new version: bumps it everywhere, checks, commits, tags and pushes. The pushed tag starts the
// Release workflow (.github/workflows/release.yml), which builds the macOS, Windows and Linux installers into a
// draft GitHub release to review and publish.
//
//   node scripts/release.mjs <x.y.z | x.y.z-rc.1 | patch | minor | major> [--dry-run] [--skip-checks] [--no-push]
//
//   --dry-run      show what would happen, change nothing
//   --skip-checks  don't run lint and tests first
//   --no-push      commit and tag locally only (push later with: git push --atomic origin main v<version>)

import { execFileSync } from "node:child_process";
import { readFileSync, writeFileSync } from "node:fs";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const BRANCH = "main";
const SEMVER = /^(\d+)\.(\d+)\.(\d+)(-[0-9A-Za-z.-]+)?$/;

const args = process.argv.slice(2);
const flag = (name) => args.includes(name);
const wanted = args.find((a) => !a.startsWith("--"));
const dryRun = flag("--dry-run");

// Files changed so far, put back if a later step fails.
const bumped = new Set();

function fail(message) {
  console.error(`release: ${message}`);
  if (bumped.size && !dryRun) {
    execFileSync("git", ["checkout", "--", ...bumped], { cwd: root });
    console.error("release: the version files were restored; nothing was committed.");
  }
  process.exit(1);
}

/** Runs a command in the repository; `capture` returns its output instead of showing it, `quiet` drops it. */
function run(cmd, cmdArgs, { capture = false, quiet = false } = {}) {
  try {
    const out = execFileSync(cmd, cmdArgs, {
      cwd: root,
      encoding: "utf8",
      stdio: capture ? ["ignore", "pipe", "pipe"] : quiet ? ["ignore", "ignore", "pipe"] : "inherit",
      // npm is npm.cmd on Windows, which needs a shell.
      shell: process.platform === "win32",
    });
    return capture ? out.trim() : "";
  } catch (e) {
    fail(`${cmd} ${cmdArgs.join(" ")} failed${e.stderr ? `:\n${e.stderr}` : ""}`);
  }
}
const git = (...a) => run("git", a, { capture: true });

function nextVersion(current, how) {
  const m = SEMVER.exec(current);
  if (!m) fail(`the current version ${current} isn't x.y.z`);
  const [major, minor, patch] = m.slice(1, 4).map(Number);
  if (how === "major") return `${major + 1}.0.0`;
  if (how === "minor") return `${major}.${minor + 1}.0`;
  // A pre-release (0.2.0-rc.1) is followed by its release (0.2.0).
  if (how === "patch") return m[4] ? `${major}.${minor}.${patch}` : `${major}.${minor}.${patch + 1}`;
  if (!SEMVER.test(how)) fail(`"${how}" is not a version (x.y.z or x.y.z-rc.1) nor patch, minor or major`);
  return how;
}

/** Replaces exactly `count` matches of `pattern` in a file (or says what would change). */
function replaceIn(file, pattern, replacement, count) {
  const path = join(root, file);
  const text = readFileSync(path, "utf8");
  const found = text.match(
    new RegExp(pattern.source, pattern.flags.includes("g") ? pattern.flags : pattern.flags + "g"),
  );
  if ((found?.length ?? 0) !== count) fail(`expected ${count} version line(s) in ${file}, found ${found?.length ?? 0}`);
  console.log(`  ${file}`);
  if (!dryRun) {
    bumped.add(file);
    writeFileSync(path, text.replace(pattern, replacement));
  }
}

if (!wanted) {
  fail("which version? e.g. `just update-version 0.2.0` (or patch, minor, major)");
}

const current = JSON.parse(readFileSync(join(root, "package.json"), "utf8")).version;
const version = nextVersion(current, wanted);
const tag = `v${version}`;
if (version === current) fail(`the version is already ${current}`);

// The release is made from an up-to-date, clean main, so the tag is exactly what CI tested.
const branch = git("rev-parse", "--abbrev-ref", "HEAD");
if (branch !== BRANCH) fail(`switch to ${BRANCH} first (on ${branch})`);
if (git("status", "--porcelain")) fail("commit or stash your changes first");
git("fetch", "--quiet", "--tags", "origin");
if (git("rev-parse", "HEAD") !== git("rev-parse", `origin/${BRANCH}`)) {
  fail(`${BRANCH} differs from origin/${BRANCH}: pull or push first`);
}
if (git("tag", "--list", tag)) fail(`the tag ${tag} exists already`);

console.log(`${dryRun ? "[dry run] " : ""}Releasing ${current} → ${version}`);

if (!flag("--skip-checks") && !dryRun) {
  console.log("Checking (lint and tests; skip with --skip-checks)…");
  run("npm", ["run", "lint"]);
  run("cargo", ["fmt", "--all", "--check"]);
  run("cargo", ["clippy", "--workspace", "--all-targets", "--", "-D", "warnings"]);
  run("npm", ["test"]);
  run("cargo", ["test", "--workspace"]);
}

console.log("Bumping the version in:");
const v = (s) => s.replace(/\$/g, "$$$$");
// package.json, and package-lock.json's own two entries (the root and packages[""]).
replaceIn("package.json", /^(  "version": ")[^"]+(")/m, `$1${v(version)}$2`, 1);
replaceIn("package-lock.json", /^(  "version": ")[^"]+(")/m, `$1${v(version)}$2`, 1);
replaceIn(
  "package-lock.json",
  /^(  "packages": \{\n    "": \{\n      "name": "cardloader",\n      "version": ")[^"]+(")/m,
  `$1${v(version)}$2`,
  1,
);
// The Cargo workspace version (the app and the TWAIN helper inherit it) and their Cargo.lock entries.
replaceIn("Cargo.toml", /(\[workspace\.package\]\nversion = ")[^"]+(")/, `$1${v(version)}$2`, 1);
replaceIn("Cargo.lock", /(name = "cardloader(?:-twain)?"\nversion = ")[^"]+(")/g, `$1${v(version)}$2`, 2);
// tauri.conf.json reads its version from package.json.
// The README's download links (the tag and the file name). They point at the draft until it is published.
const was = current.replace(/\./g, "\\.");
replaceIn(
  "README.md",
  new RegExp(`(releases/download/v)${was}(/Cardloader[-_])${was}([-_])`),
  `$1${v(version)}$2${v(version)}$3`,
  6,
);

if (dryRun) {
  console.log(`Would commit "Release ${tag}", tag ${tag} and push ${BRANCH} and ${tag} to origin.`);
  process.exit(0);
}

// The lockfiles must still match their manifests after the edit.
run("cargo", ["metadata", "--format-version", "1", "--locked"], { quiet: true });

run("git", ["add", "package.json", "package-lock.json", "Cargo.toml", "Cargo.lock", "README.md"]);
run("git", ["commit", "--quiet", "-m", `Release ${tag}`]);
run("git", ["tag", "-a", tag, "-m", `Cardloader ${tag}`]);
console.log(`Committed and tagged ${tag}.`);

if (flag("--no-push")) {
  console.log(`Not pushed. When ready: git push --atomic origin ${BRANCH} ${tag}`);
  process.exit(0);
}
// Atomic: the branch and the tag land together or not at all.
run("git", ["push", "--atomic", "origin", BRANCH, tag]);

const remote = git("remote", "get-url", "origin");
const github = /github\.com[:/](.+?)(?:\.git)?$/.exec(remote)?.[1];
console.log(`\nPushed ${tag}. The Release workflow now builds the installers (about 30 minutes)`);
if (github) {
  console.log(`  https://github.com/${github}/actions/workflows/release.yml`);
  console.log(`Then review the draft release, edit its notes and publish it:\n  https://github.com/${github}/releases`);
} else {
  console.log("Then review the draft release on GitHub, edit its notes and publish it.");
}
