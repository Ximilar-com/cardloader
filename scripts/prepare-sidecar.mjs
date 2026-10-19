#!/usr/bin/env node
// Builds the Windows TWAIN sidecar and puts it, with the 32-bit TWAINDSM.dll, where Tauri bundles it
// (src-tauri/binaries/). The helper is always built 32-bit: the default RICOH PaperStream IP TWAIN driver is
// 32-bit, and a TWAIN driver only loads into a process of the same bitness. Does nothing off Windows.
//
//   node scripts/prepare-sidecar.mjs [--target <rust target triple of the app>] [--force]

import { execFileSync } from "node:child_process";
import { createHash } from "node:crypto";
import { copyFileSync, existsSync, mkdirSync, readFileSync, rmSync, writeFileSync } from "node:fs";
import { tmpdir } from "node:os";
import { dirname, join } from "node:path";
import { fileURLToPath } from "node:url";

const root = join(dirname(fileURLToPath(import.meta.url)), "..");
const binaries = join(root, "src-tauri", "binaries");
const HELPER_TARGET = "i686-pc-windows-msvc";
const DSM_URL = "https://github.com/twain/twain-dsm/releases/download/v2.5.1/dsm_020501.zip";
const DSM_SHA256 = "6d5b520e0d4d524a195aa6ed749edb09d7118eed96c7defb4952bfcdb1590cee";
const DSM_ENTRY = "dsm_020501/windows/32/TWAINDSM.dll";

const args = process.argv.slice(2);
const force = args.includes("--force");
const targetArg = args.includes("--target") ? args[args.indexOf("--target") + 1] : undefined;

if (process.platform !== "win32" && !force) {
  console.log("prepare-sidecar: not on Windows, nothing to do");
  process.exit(0);
}

const run = (cmd, cmdArgs) => execFileSync(cmd, cmdArgs, { cwd: root, stdio: "inherit" });
const hostTriple = () =>
  execFileSync("rustc", ["-vV"], { encoding: "utf8" })
    .split("\n")
    .find((l) => l.startsWith("host:"))
    .slice(5)
    .trim();

// Tauri looks for `<name>-<target triple of the app>.exe`; the helper inside is 32-bit regardless.
const appTarget = targetArg ?? process.env.TAURI_ENV_TARGET_TRIPLE ?? hostTriple();
mkdirSync(binaries, { recursive: true });

run("rustup", ["target", "add", HELPER_TARGET]);
run("cargo", ["build", "--release", "-p", "cardloader-twain", "--target", HELPER_TARGET]);
const built = join(root, "target", HELPER_TARGET, "release", "cardloader-twain.exe");
const dest = join(binaries, `cardloader-twain-${appTarget}.exe`);
copyFileSync(built, dest);
console.log(`prepare-sidecar: ${dest}`);

const dll = join(binaries, "TWAINDSM.dll");
if (existsSync(dll) && !force) {
  console.log("prepare-sidecar: TWAINDSM.dll already present");
} else {
  const response = await fetch(DSM_URL);
  if (!response.ok) throw new Error(`download failed: ${response.status} ${DSM_URL}`);
  const zip = Buffer.from(await response.arrayBuffer());
  const sha = createHash("sha256").update(zip).digest("hex");
  if (sha !== DSM_SHA256) throw new Error(`TWAINDSM checksum mismatch: ${sha}`);
  const work = join(tmpdir(), `cardloader-dsm-${process.pid}`);
  mkdirSync(work, { recursive: true });
  writeFileSync(join(work, "dsm.zip"), zip);
  // bsdtar (tar.exe on Windows 10+, macOS) reads zip archives.
  execFileSync("tar", ["-xf", "dsm.zip", DSM_ENTRY], { cwd: work, stdio: "inherit" });
  writeFileSync(dll, readFileSync(join(work, DSM_ENTRY)));
  rmSync(work, { recursive: true, force: true });
  console.log(`prepare-sidecar: ${dll} (TWAIN DSM 2.5.1, LGPL-2.1)`);
}
