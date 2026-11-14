#!/usr/bin/env node
// Writes the catalog entry of a local model from its Hugging Face repository: the pinned revision, the sizes and
// the SHA-256 of its two files. Paste what it prints into MODELS in src-tauri/src/local/catalog.rs and fill in
// the lines marked TODO (README: "Adding another model").
//
//   node scripts/model-entry.mjs <owner/repo>                              list the repository's GGUF files
//   node scripts/model-entry.mjs <owner/repo> <model.gguf> <mmproj.gguf>   print the entry
//
//   --rev <commit>  pin this revision instead of the newest

const args = process.argv.slice(2);
const revAt = args.indexOf("--rev");
const rev = revAt >= 0 ? args.splice(revAt, 2)[1] : null;
const [repo, modelFile, mmprojFile] = args;

function fail(message) {
  console.error(`model-entry: ${message}`);
  process.exit(1);
}

if (!repo || !/^[\w.-]+\/[\w.-]+$/.test(repo) || (modelFile && !mmprojFile) || (revAt >= 0 && !rev)) {
  fail("usage: node scripts/model-entry.mjs <owner/repo> [<model.gguf> <mmproj.gguf>] [--rev <commit>]");
}

const size = (bytes) => (bytes >= 1e9 ? `${(bytes / 1e9).toFixed(1)} GB` : `${Math.round(bytes / 1e6)} MB`);
const digits = (n) => String(n).replace(/\B(?=(\d{3})+$)/g, "_");
const isMmproj = (name) => /mmproj/i.test(name);
// A model too large for one file is split into parts, which the catalog can't hold.
const isPart = (name) => /-\d{5}-of-\d{5}\.gguf$/.test(name);

const api = `https://huggingface.co/api/models/${repo}${rev ? `/revision/${rev}` : ""}?blobs=true`;
const response = await fetch(api).catch((e) => fail(`cannot reach huggingface.co: ${e.message}`));
if (response.status === 401 || response.status === 404) {
  fail(`${repo}${rev ? ` at ${rev}` : ""} isn't a public model repository on Hugging Face`);
}
if (!response.ok) fail(`huggingface.co answered ${response.status}`);
const info = await response.json();
if (info.gated) {
  fail(`${repo} asks for a login to download (gated); the app downloads without one. Look for a public copy.`);
}

const files = (info.siblings ?? []).filter((f) => f.rfilename.endsWith(".gguf"));
if (files.length === 0) fail(`${repo} has no GGUF files; look for the model's "-GGUF" repository`);
const card = info.cardData ?? {};

if (!modelFile) {
  const list = (title, rows) => {
    console.log(`\n${title}`);
    if (rows.length === 0) console.log("  none");
    const width = Math.max(...rows.map((f) => f.rfilename.length));
    for (const f of rows) {
      console.log(
        `  ${f.rfilename.padEnd(width)}  ${size(f.size).padStart(8)}${isPart(f.rfilename) ? "  (a part)" : ""}`,
      );
    }
  };
  console.log(`${repo} @ ${info.sha}${card.license ? `, license: ${card.license}` : ""}`);
  list(
    "Models",
    files.filter((f) => !isMmproj(f.rfilename)),
  );
  list(
    "Vision projectors (mmproj)",
    files.filter((f) => isMmproj(f.rfilename)),
  );
  if (!files.some((f) => isMmproj(f.rfilename))) {
    console.log("\nWithout an mmproj file the model can't read pictures: this repository won't work.");
  } else {
    console.log(`\nNext: node scripts/model-entry.mjs ${repo} <model.gguf> <mmproj.gguf>`);
    console.log("Q4_K_M is the usual model file; take the mmproj as F16 or Q8 (a smaller one reads worse).");
  }
  process.exit(0);
}

const pick = (name) => {
  const file = files.find((f) => f.rfilename === name);
  if (!file) fail(`${repo} has no file ${name}; run without file names to list them`);
  if (isPart(name)) {
    fail(`${name} is one part of a split model; the catalog takes a model in one file. Choose a smaller quant.`);
  }
  if (name.includes("/")) fail(`${name} is in a subfolder; the catalog takes files from the repository's top folder`);
  if (!file.lfs?.sha256) fail(`Hugging Face gave no SHA-256 for ${name}`);
  return { name, size: file.lfs.size ?? file.size, sha256: file.lfs.sha256 };
};
const model = pick(modelFile);
const mmproj = pick(mmprojFile);
if (isMmproj(model.name) || !isMmproj(mmproj.name)) {
  console.error("model-entry: check the order: the model file first, then its mmproj\n");
}

// "gemma-3-4b-it-GGUF" → base "gemma-3-4b-it", parameters "4B".
const base = repo.split("/")[1].replace(/[-_.]?gguf$/i, "");
const sized = /(?:^|[-_ ])(\d+(?:\.\d+)?[bm])(?=$|[-_ ])/i;
const params = base.match(sized)?.[1].toUpperCase();
// The name ends with the parameter count: "gemma-3-it 4B".
const name = params ? `${base.replace(sized, "").replace(/^[-_ ]+/, "")} ${params}` : base;
const id = base.toLowerCase().replace(/[^a-z0-9.-]+/g, "-");
const quant = model.name.match(/(Q\d[\w]*|BF16|F16|F32)(?=\.gguf$)/i)?.[1];
// Its files plus room to work, as a GPU with its own memory needs.
const memoryMb = Math.ceil(((model.size + mmproj.size) * 1.1) / 1e6 / 100) * 100 + 500;

const licenses = {
  "apache-2.0": ["Apache License 2.0", "https://www.apache.org/licenses/LICENSE-2.0"],
  mit: ["MIT License", "https://opensource.org/license/mit"],
};
const known = licenses[card.license];
const homepage = `https://huggingface.co/${repo}`;
const licenseName = known?.[0] ?? card.license_name ?? card.license ?? "";
const licenseUrl = known?.[0] ? known[1] : (card.license_link ?? homepage);
const text = (s) => JSON.stringify(String(s));
const file = (f) => `ModelFile {
            name: ${text(f.name)},
            url: hf!(${text(repo)} @ ${text(info.sha)} / ${text(f.name)}),
            size: ${digits(f.size)},
            sha256: ${text(f.sha256)},
        }`;

console.log(`    ModelSpec {
        id: ${text(id)},
        // TODO: the name people see; it ends with the parameter count.
        name: ${text(name)},
        // "Fast", "Balanced", "Accurate" or "Large".
        tier: "Large",
        params: ${text(params ?? "TODO")},
        // TODO: what it is good at and what computer it wants, once you have tried it.
        description: ${text(`${base}${quant ? ` (${quant})` : ""}.`)},
        // An estimate (its files plus room to work): measure it.
        memory_mb: ${digits(memoryMb)},
        // TODO: who made the model (the makers of the original, not of the GGUF files).
        maker: ${text(repo.split("/")[0])},
        // TODO: check the license on the original model's page${known ? "" : "; Hugging Face says: " + (card.license ?? "nothing")}.
        license_name: ${text(licenseName || "TODO")},
        license_url: ${text(licenseUrl)},
        // TODO: true if the license limits commercial use.
        commercial_limit: false,
        homepage: ${text(homepage)},
        profile: Profile::GENERAL,
        model: ${file(model)},
        mmproj: ${file(mmproj)},
    },`);
