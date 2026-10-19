# Third-party components

## TWAIN Data Source Manager (Windows builds only)

The Windows installer ships an unmodified 32-bit `TWAINDSM.dll` 2.5.1 from the TWAIN Working Group, next to the
`cardloader-twain` helper, which loads it dynamically.

- Source and release: <https://github.com/twain/twain-dsm> (release `v2.5.1`, `dsm_020501.zip`)
- License: GNU Lesser General Public License v2.1 — <https://github.com/twain/twain-dsm/blob/master/LICENSE>

`scripts/prepare-sidecar.mjs` downloads it at build time and verifies its SHA-256.

## llama.cpp (local model support)

Builds with the default `local-model` feature compile [llama.cpp](https://github.com/ggml-org/llama.cpp) into the
app through the `llama-cpp-2` / `llama-cpp-sys-2` crates.

- License: MIT — <https://github.com/ggml-org/llama.cpp/blob/master/LICENSE>

## Local models (not bundled)

Local models are **not** part of cardloader or its installers. When the user chooses one in Settings and accepts
its license, the app downloads it from Hugging Face (pinned revision, SHA-256 verified).

All of them are by Liquid AI, under the [LFM Open License v1.0](https://www.liquid.ai/lfm-license) (commercial use
limited by a revenue threshold):

- LFM2.5-VL-450M — <https://huggingface.co/LiquidAI/LFM2.5-VL-450M-GGUF>
- LFM2.5-VL-1.6B — <https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF>
- LFM2-VL-1.6B — <https://huggingface.co/LiquidAI/LFM2-VL-1.6B-GGUF>
- LFM2-VL-3B — <https://huggingface.co/LiquidAI/LFM2-VL-3B-GGUF>

## Rust and npm dependencies

Rust crates and npm packages are used under their own licenses (MIT, Apache-2.0 and similar). List them with
`cargo tree` / `npm ls`, or generate a report with tools such as `cargo-about` or `license-checker`.
