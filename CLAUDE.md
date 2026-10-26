# CLAUDE.md

Guidance for AI coding agents working on the Cardloader desktop app. All paths and commands below are
relative to the repository root. Product overview and user docs: [README.md](README.md).

**This is a public open-source repository.** Never commit API tokens, credentials, customer data, real card scans
that aren't yours to publish, personal file paths or internal URLs/hostnames. Test data and examples use made-up
values (`demo@example.com`, `/Users/demo/...`).

## Stack

- Tauri 2 desktop app: Rust backend in `src-tauri/` (crate `cardloader`, lib `cardloader_lib`), React 19 + TypeScript
  + Vite + Tailwind v4 frontend in `src/`.
- Cargo workspace (`Cargo.toml` here) with `src-tauri` and `crates/cardloader-twain` (Windows TWAIN sidecar, built
  32-bit). Rust toolchain is pinned in `rust-toolchain.toml`.
- npm (not pnpm/yarn). Node 22+.
- The default cargo feature `local-model` compiles llama.cpp (`llama-cpp-2`, pinned, with `mtmd` vision support):
  it needs cmake + clang. `--no-default-features` builds without it (local models then report "unsupported").

## Commands

```sh
just setup   # npm ci + cargo fetch
just dev     # run the app (tauri dev)
just lint    # eslint + prettier --check + tsc, cargo fmt --check, clippy -D warnings — must pass
just test    # vitest + cargo test --workspace — must pass
just fmt     # cargo fmt + prettier --write
just update-version 0.2.0   # release: bump, lint + test, commit, tag, push (--dry-run to preview)
npm run dev  # UI only, in a browser, with demo data (src/lib/demo.ts) — use it to check UI changes
```

Single Rust test: `cargo test -p cardloader <name>`. A `Makefile` mirrors the Justfile targets. CI
(`.github/workflows/ci.yml`) runs lint + test on Linux, macOS and Windows.

## Architecture

```
src-tauri/src/
  lib.rs          setup (db, settings, runner) + the command list in generate_handler!
  commands.rs     every #[tauri::command] the UI calls
  runner.rs       session runner: source → ingest (thumbnails) → recognise → grade (async jobs) → events;
                  pick_match, CSV import; end-to-end tests against wiremock at the bottom
  sync.rs         mirroring a session to Ximilar Cards Cloud (batches, images, PATCHed match corrections)
  db.rs           SQLite (rusqlite, one Mutex<Connection>); migrations/NNN_*.sql applied by PRAGMA user_version
  model.rs        serde types shared with the UI (camelCase on the wire)
  settings.rs     settings.json + secrets (API token, eBay Cert ID and login) in the OS keychain (keyring)
  imaging.rs      decode, EXIF orientation, thumbnails/previews, JPEG/base64 for upload
  export.rs       CSV export
  source/         folder, watch folder (notify), csv_file.rs (import parser), scanner/{sane.rs, twain.rs}
  ximilar/        http.rs (client, retry, error classes), recognition.rs, grading.rs, cards_cloud.rs
  ebay/           selling on eBay with the user's keyset: mod.rs (OAuth: consent window, tokens, account setup),
                  client.rs (REST calls, errors[]), listing.rs (card → category, condition descriptors, title,
                  item specifics, inventory item + offer), sync.rs (jobs: drafts, publish, refresh, remove)
  market/         files for other marketplaces: tcgplayer.rs (list of ungraded cards), whatnot.rs (bulk listing
                  CSV), shopify.rs (product CSV with condition variants, inventory CSV), imgbb.rs (public picture
                  hosting, `Hosting`); shares eBay's card typing and titles
  local/          on-device identification: catalog.rs (pinned model files + SHA-256), download.rs (resumable),
                  llama.rs (llama.cpp engine, feature-gated), identify.rs (prompts, GBNF grammars, parsing),
                  compare.rs (agreement with Ximilar), mod.rs (LocalModels: status, downloads, loaded model)
src/
  lib/api.ts      TypeScript types + wrappers for every command and event
  lib/cardview.ts one Row/CardView shape for local items and Cards Cloud rows
  lib/demo.ts     mockIPC answers for `npm run dev` outside Tauri
  lib/setup.ts    first-run setup answers ↔ the default settings they become (only the settings are stored)
  pages/          DashboardPage (home: 7-day bars, activity heatmap, last sessions, defaults), SessionsPage
                  (local + Cards Cloud tabs), SessionPage, RemoteSessionPage, SettingsPage
  components/     SetupWizard (shown while `setupDone` is false; rerun from Home or Settings), SessionTable
                  (virtualized), DetailPanel, NewSessionDialog, SessionActions, Menu, …
```

Events from Rust: `session-updated` (Session), `item-updated` (ItemSummary), `model-updated` (ModelStatus, download
progress), `compare-progress` and `ebay-progress` (EbayProgress); pages patch their state from them.

## Rules for changes

- **A new or changed command touches four places:** the function in `commands.rs`, `generate_handler!` in `lib.rs`,
  the wrapper and types in `src/lib/api.ts`, and a mock in `src/lib/demo.ts`. Types are written by hand; keep
  Rust (`#[serde(rename_all = "camelCase")]`) and TypeScript in sync.
- **Schema changes** go in a new `src-tauri/migrations/NNN_name.sql` appended to `MIGRATIONS` in `db.rs`. Never edit a
  released migration.
- **Pipeline changes** (runner, sync, sources, Ximilar clients) need a test; extend the wiremock end-to-end tests in
  `runner.rs` (`folder_session_end_to_end`, `csv_import_end_to_end`) rather than calling real APIs.
- New Tauri plugin APIs need a permission in `src-tauri/capabilities/default.json`.
- Match the surrounding style: Rust via `cargo fmt`, frontend via Prettier (`.prettierrc.json`, width 120). Comments
  explain why, not what. User-facing text is plain and short.
- Use `components/Select` (icons, hints, badges; fixed-positioned so dialogs don't clip it), not a native `<select>`.
- ESLint runs `react-hooks` v7: no synchronous `setState` in effects (derive state, use keyed remounts, or set it in
  async callbacks).

## Domain notes and gotchas

- Recognition endpoints (`collectibles/v2/{tcg_id,sport_id,comics_id,slab_id}`) are synchronous; **Auto** is one
  `/analyze` call, read as the type its tags say (`route_from_tags`). `/analyze` takes neither `magic_ai` nor a back
  record, so only a sports card wanting those is then sent to sport_id too (two calls). The main card is the largest identified object that isn't a slab label — Rust
  (`recognition::ident_pointer`) and TS (`cardview::matchOf`) must choose the same way.
- **Front and back** (`both_sides`): the back preview is a second record (`_id: "back"`) in the same request, for
  sport_id and custom endpoints only (`Endpoint::reads_backs`). The card is the side whose best match is closest
  (front on a tie, `ident_pointer` / `matchOf`); a back with an error status doesn't fail the card. `magic_ai` is a
  sport_id root flag (≤ 2 records); a response with `magic_ai_used` counts one more call. A custom endpoint path
  replaces the standard path and is read as the session's type, so stored recognitions carry `id_type`
  (`Endpoint::of_raw`).
- **Event ids** of folder, watch-folder and CSV cards are `source::file_key(front)`, a hash of the front image's file
  name (scans keep random ids: every scanner session names them `scan_0001…`; a name already used in the session gets
  a random id). A session linked to Cards Cloud lists the remote rows' `event_id`s before a run and skips the files
  with those keys (`source::files_with_keys`), which is how `create_session(remoteSession)` continues a session.
- The API token lives in the OS keychain, except in macOS debug builds (`settings::USE_KEYCHAIN`): each rebuild is a
  new unsigned binary, so the keychain would ask for the password on every launch. They keep it in `settings.json`.
- **Skip detection** (`skip_detection`): each record carries `_objects: [{"name": "Card", "prob": 1.0, "bound_box":
  [0, 0, w, h]}]` sized to the image actually sent (the preview), as the Cardstream client sends pre-cropped cards; the
  endpoint then identifies that box instead of detecting one.
- **Known attributes** ("Faster and more accurate" in the dialog: `known_category`, `known_side`, `known_rotation`,
  `alphabet` for TCG, `subcategory` = game or sport as the taxonomy spells it, e.g. `Pokemon`, `YuGiOh!`; lists in
  `recognition::TCG_GAMES` / `SPORTS`, mirrored in `NewSessionDialog.tsx`) go on the record, as Ximilar's docs show, or on that card object when detection is skipped. The
  category is the session type's (`Endpoint::category_attrs`), never in Auto (`detects_type`). Unless the
  images are known to be upright, the request has root `"rotate": true`: without it the endpoints tag a turned card
  (`Rotation: rotated_upside_down`) but identify it as it lies.
- Confidence tiers on the best-match distance: ≤ 0.30 high, ≤ 0.40 medium, else low (duplicated in `cardview.ts`).
- Grading (`account/v2/request/`, type `card-grader`) is asynchronous: submit, poll status, fetch the result; result
  image URLs are temporary, so overlays are downloaded into the session folder.
- Cards Cloud (`cards-cloud/v2`, formerly `cardstream/v2`) is shared with Ximilar's Cardstream live-stream app;
  a remote session's `client.name` / `platform` tells which app made it (Cardloader sends `client.name: "cardloader"`).
- Cards Cloud rows are keyed by `event_id`. Re-uploading a known row never changes its card fields, so a user's
  "Select" of another candidate is sent as `PATCH identification/{id}/` (`sync::push_picks`), tracked by
  `picks`/`picks_synced`.
- Items: `rev` counts result changes, `synced_rev` what Cards Cloud has. Imported CSV cards start at `rev = 1`.
- The Windows scanner path never runs TWAIN in the app process: `cardloader-twain` is a sidecar speaking JSON lines
  over stdio. It must be built for `i686-pc-windows-msvc` (`scripts/prepare-sidecar.mjs`).
- Vite 8 no longer bundles esbuild; `vite.config.ts` uses `minify: !process.env.TAURI_ENV_DEBUG`.
- **Local model** (`Engine::Local`): two generations per card — whole card (name, year, game) and the enlarged bottom
  strip (collector number, set code); a 450M model reads small print only when it is enlarged, and fills a "set name"
  with random card text, so that field isn't asked for. Answers are constrained by hand-written GBNF grammars (field
  order matters; a schema-generated grammar sorts keys). Local items have `confidence = "local"`, no distance or
  price, `calls = 0`, and `recognition.endpoint = "local/<model>"`; concurrency is 1.
- **Local grading** (`local::grade`): one more generation per side after identification, stored in Ximilar's shape
  (`records[0].grades`, `records[0]._objects[0].Condition`) so the UI reads both alike; the card takes the worse side.
  `condition_mode` names a scale (`tcgplayer`, `cardmarket`, `ebay_tcg`, `ebay_sport`; Ximilar gets `ebay` for both
  eBay ones). Small models echo the numbers a prompt suggests: spell out every step of a numeric scale, and expect
  condition labels to be more useful than numeric grades (`real_model_grades_cards`).
- **Adding a model**: append a `ModelSpec` to `local::catalog::MODELS` (smallest first) with a pinned revision
  (`https://huggingface.co/api/models/<repo>` → `sha`), file sizes and SHA-256 (`.../tree/main` → `lfs.oid`), a GGUF
  quant plus its `mmproj`. Measure it with the opt-in test (`CARDLOADER_MODEL=<id>`) and its peak memory before
  writing its description; mirror it in `src/lib/demo.ts`.
- `llama-cpp-2`'s `LlamaSampler::sample` already accepts the token; accepting it again corrupts the grammar
  (`GGML_ASSERT(!stacks.empty())`). The llama.cpp backend can be initialised once per process (`OnceLock`).
- Real-model checks are opt-in (`#[ignore]`): `real_model_reads_cards` (needs `CARDLOADER_MODELS`, `CARDLOADER_CARDS`)
  and `real_download` (`CARDLOADER_DOWNLOAD_TO`), both for `CARDLOADER_MODEL` (default the 450M model). Run them with `--release -- --ignored --nocapture`. Never commit
  model files or card images.
- **eBay** (`ebay/`): the user's own keyset (App ID, RuName in settings; Cert ID and the refresh token are
  `settings::Secret`s like the API token; changing the keyset or environment disconnects). eBay redirects only to
  https pages, so consent opens in a webview window (`ebay-login`, in no capability) and the code is read from its
  navigation (`consent_result`); pasting the final address is the fallback. Access tokens live in memory
  (`EbayAuth`). A card becomes an inventory item (SKU `cl-<session[..8]>-<event_id>`) plus an **unpublished offer =
  a draft**; publishing is a separate job. Bulk calls take ≤ 25 and answer per card (HTTP 207): a card's error stays
  on the card (`items.ebay_error`), login/network failures stop the job (`sessions.ebay_error`). TCG → 183454, sports
  → 261328, comics not listed; graded only from a slab label (descriptors 27501/27502/27503), else ungraded with
  40001 (value ids differ per category). Item specific names are not verified against eBay's taxonomy yet.
  Marketplace: EBAY_US only (`listing::MARKETPLACES`). One eBay job per session (`Runner::ebay_start`).
- **TCGplayer / Whatnot exports** (`market/`): TCGplayer gives no API to new apps and its "Import to Staged" CSV
  matches rows only by its SKU ids (product + condition + printing + language), which we don't have — so the
  TCGplayer export is a list for Seller Portal (product line, set, name, number, TCGplayer condition, price, and the
  product id from Ximilar's `tcgplayer.com/product/{id}` link). Graded cards (Listings with Photos only) and sports
  cards (no such line) are left out. Whatnot's CSV is its US template's 21 columns in order, with its exact category,
  sub category and condition strings (the condition scale differs for sports singles); prices are whole dollars
  rounded up; no BOM (it would be read into the first header). Pictures must be public URLs: they go to ImgBB with the
  user's key (`Secret::ImgbbKey`) once per card (`items.hosted_images`). Cards whose condition Ximilar graded keep it
  (`CardCondition::read`). Shopify: CSV only (no Admin API: since 2026 a store token needs a Dev Dashboard app). The
  product file uses the 2024 header names for the columns we fill; ungraded copies of a card share a product whose
  handle comes from game/set/name/number (stable across sessions), conditions are `Option1` "Condition" variants;
  slabs and "one product per scanned card" get their own handle and a `Title`/`Default Title` variant. The inventory
  file matches rows by handle + option value and sets `On hand (new)` at a named location.
- The browser demo logs `[TAURI] Couldn't find callback id` warnings from its mocked events; they are harmless.

## Not verified automatically

Real Ximilar API calls, real eBay calls (use a sandbox keyset), real scanners (SANE/TWAIN), signed release builds, and
the real local model (CI uses a fake `VisionModel`; see the opt-in tests above). Say so when a change depends on
them, and describe how it was checked.
