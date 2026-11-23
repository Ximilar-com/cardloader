#!/usr/bin/env node
// Fails when a Windows binary we ship needs a DLL we don't ship. llama.cpp is built as shared libraries
// (`gpu-backends`), and its build links what it finds on the build machine: v0.6.0 to v0.8.0 shipped a
// llama-common.dll that needed OpenSSL's libssl-3-x64.dll and libcrypto-3-x64.dll, so the app started only on PCs
// that had those from other software (`.cargo/config.toml` is the fix; this is what notices the next one). The
// Visual C++ runtime counts too: build.rs ships the 64-bit DLLs, and the 32-bit TWAIN sidecar must not ask for them.
//
// Reads the import tables itself, so it runs on any OS and needs no Visual Studio tools.
//
//   node scripts/check-dll-imports.mjs <file or folder>...     (a folder: its .dll and .exe files)

import { readdirSync, readFileSync, statSync } from "node:fs";
import { basename, join } from "node:path";

// Part of every Windows 10 and 11. A new name here must be one Windows itself provides.
const WINDOWS = new Set(
  `advapi32 bcrypt bcryptprimitives cfgmgr32 combase comctl32 comdlg32 crypt32 d3d11 dbghelp dnsapi dwmapi dwrite
   dxgi gdi32 imm32 iphlpapi kernel32 mswsock ncrypt netapi32 ntdll ole32 oleaut32 pdh powrprof propsys psapi
   rpcrt4 secur32 setupapi shcore shell32 shlwapi user32 userenv uxtheme version winhttp winmm winspool.drv ws2_32
   wtsapi32`
    .split(/\s+/)
    .map((name) => (name.includes(".") ? name : `${name}.dll`)),
);
// Windows' API sets and the Universal C Runtime (Windows 10 and later). The Visual C++ runtime (msvcp140.dll,
// vcruntime140.dll, vcomp140.dll, …) is not Windows': build.rs ships the DLLs llama.cpp's libraries need.
const API_SET = /^(api|ext)-ms-/;
// The GPU driver's Vulkan loader. Only ggml-vulkan.dll needs it, a backend module that is loaded at runtime:
// without the driver it doesn't load and the app runs on the CPU.
const OPTIONAL = new Set(["vulkan-1.dll"]);

const MACHINES = { 0x14c: "32-bit", 0x8664: "64-bit", 0xaa64: "ARM64" };

/**
 * What kind of PC a PE file is for and the DLLs it names in its import and delay-import tables, lower case; null
 * when it isn't a PE file.
 */
function read(file) {
  const b = readFileSync(file);
  if (b.length < 0x40 || b.readUInt16LE(0) !== 0x5a4d) return null;
  const pe = b.readUInt32LE(0x3c);
  if (pe + 24 > b.length || b.readUInt32LE(pe) !== 0x4550) return null;
  const sectionCount = b.readUInt16LE(pe + 6);
  const optional = pe + 24;
  const plus = b.readUInt16LE(optional) === 0x20b; // PE32+ (64-bit)
  const directories = optional + (plus ? 112 : 96);
  const directoryCount = b.readUInt32LE(directories - 4);
  const sections = optional + b.readUInt16LE(pe + 20);

  const offset = (rva) => {
    for (let i = 0; i < sectionCount; i++) {
      const s = sections + i * 40;
      const start = b.readUInt32LE(s + 12);
      const size = Math.max(b.readUInt32LE(s + 8), b.readUInt32LE(s + 16));
      if (rva >= start && rva < start + size) return rva - start + b.readUInt32LE(s + 20);
    }
    return null;
  };
  const text = (rva) => {
    const at = offset(rva);
    if (at === null) return null;
    return b.toString("latin1", at, b.indexOf(0, at)).toLowerCase();
  };
  // Each table is a list of fixed-size entries ending with an empty one; `name` is where an entry keeps the RVA
  // of the DLL's name.
  const table = (index, entrySize, name) => {
    if (index >= directoryCount) return [];
    const rva = b.readUInt32LE(directories + index * 8);
    let at = rva ? offset(rva) : null;
    const names = [];
    for (; at !== null && at + entrySize <= b.length; at += entrySize) {
      const nameRva = b.readUInt32LE(at + name);
      if (!nameRva) break;
      const dll = text(nameRva);
      if (dll) names.push(dll);
    }
    return names;
  };
  const machine = b.readUInt16LE(pe + 4);
  return {
    machine: MACHINES[machine] ?? `machine 0x${machine.toString(16)}`,
    needs: [...new Set([...table(1, 20, 12), ...table(13, 32, 4)])],
  };
}

const paths = process.argv.slice(2);
if (!paths.length) {
  console.error("usage: node scripts/check-dll-imports.mjs <file or folder>...");
  process.exit(2);
}
const files = paths.flatMap((path) =>
  statSync(path).isDirectory()
    ? readdirSync(path)
        .filter((name) => /\.(dll|exe)$/i.test(name))
        .map((name) => join(path, name))
    : [path],
);
const binaries = files.map((file) => ({ name: basename(file), ...read(file) })).filter((binary) => binary.needs);
if (!binaries.length) {
  console.error(`check-dll-imports: no Windows binaries in ${paths.join(", ")}`);
  process.exit(1);
}
// A 32-bit program (the TWAIN sidecar) can't load a 64-bit DLL of the name it asks for, and finds the one beside
// it first: a shipped DLL counts only for programs of its own kind.
const shipped = new Set(binaries.map((binary) => `${binary.machine} ${binary.name.toLowerCase()}`));
// What is missing → the binaries that need it.
const missing = new Map();
for (const { name, machine, needs } of binaries) {
  for (const dll of needs) {
    if (shipped.has(`${machine} ${dll}`) || WINDOWS.has(dll) || API_SET.test(dll) || OPTIONAL.has(dll)) continue;
    const other = binaries.find((binary) => binary.name.toLowerCase() === dll);
    const what = `${dll} for ${machine} programs${other ? ` (the one shipped is ${other.machine})` : ""}`;
    missing.set(what, [...(missing.get(what) ?? []), name]);
  }
}
if (missing.size) {
  console.error("check-dll-imports: these DLLs are neither shipped nor part of Windows:");
  for (const [what, by] of missing) console.error(`  ${what}: needed by ${by.join(", ")}`);
  console.error("Build without them, ship them, or add a DLL that Windows provides to this script's list.");
  process.exit(1);
}
console.log(`check-dll-imports: ${binaries.length} binaries need nothing we don't ship`);
