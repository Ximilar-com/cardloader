<p align="center">
  <img src="src-tauri/icons/128x128@2x.png" width="96" alt="Cardloader" />
</p>

<h1 align="center">Cardloader</h1>

<p align="center">
  Scan trading, sports and comic cards from a folder or a document scanner, identify and grade them with the
  <a href="https://docs.ximilar.com/collectibles/recognition">Ximilar API</a>, and keep every run as a session —
  on your computer and in Ximilar Cards Cloud.
</p>

---

Cardloader is an open-source desktop app for Windows, macOS and Linux, built with [Tauri 2](https://tauri.app)
(Rust) and React.

- **Home and setup.** On first launch, five questions (what you scan, which game, whether you grade, check
  condition or want prices, how cards come in, and whether the Ximilar API or a local model identifies them) set
  the defaults of new sessions; run them again any time from Home or Settings. Home shows the cards analysed in the
  last 7 days, a year of activity, the last sessions and those defaults.
- **Sessions.** Each analysis of a folder, or each run of the scanner, is a session. The table fills in as
  results arrive; click a card to see its images, the best match, price statistics, grading and links. If the
  match is wrong, select one of the other candidates instead; the correction is also sent to Cards Cloud.
- **Identification.** The [Collectibles Recognition](https://docs.ximilar.com/collectibles/recognition) endpoints
  identify the cards: TCG ID (Pokémon, Magic, Yu-Gi-Oh!, One Piece, Lorcana and more), Sport ID, Comics ID and Slab
  ID. In **Auto** mode, `/analyze` detects the card type and identifies the card in one call. Rotated and
  upside-down cards are turned upright before they are identified, unless the session says every card is upright.
- **Front and back.** In a folder, images named `card_F.jpg` and `card_B.jpg` (or `_front`/`_back`, any image
  format) are paired by name; other images pair with the next one. When a session is set to **Identify from the front
  and the back**, the back goes to recognition together with the front. This helps sports cards, whose backs carry
  the player, year and number; TCG and comics identification reads the front only, because their backs look alike.
  **Magic AI** lets Sport ID identify cards that aren't in Ximilar's database (extra credits, only when it runs).
  A **custom endpoint** that Ximilar set up for you can replace the standard one; it gets the same request.
- **Local model (optional).** Cards can instead be identified on your computer, offline and for free, by a
  vision language model from Liquid AI's LFM family (450M to 3B parameters, about 1–4 s per card). It reads the
  name, collector number and year printed on the card; it can't give prices, other candidates or grades. See
  [Local model](#local-model).
- **Grading.** [Card Grading](https://docs.ximilar.com/collectibles/card-grading) can grade each card in full
  (corners, edges, surface, centering, final grade) using both sides, check its condition, or measure centering
  only. The grading overlays are saved alongside the session.
- **Scanner.** Cardloader drives a RICOH/Fujitsu **fi-8170**, or any SANE/TWAIN document scanner, in duplex batches.
  Front and back are paired automatically, in the order you feed them (front or back first), and pages fed upside
  down can be turned as they are scanned. On Windows, **Settings → Scanner → Driver settings…** opens the scanner's
  own options (such as automatic cropping and deskew), and every scan keeps them.
- **Watch folder.** Cardloader can also analyse images as another app saves them into a folder, for example
  PaperStream Capture.
- **Cards Cloud.** Sessions can be mirrored live to
  [Ximilar Cards Cloud](https://app.ximilar.com/login?open=cards-cloud). Each card is uploaded with its image,
  match, prices and grading, and the Cards Cloud tab lets you browse what is stored there.
- **Continue a session.** Paste a Cards Cloud session's ID (or link) into the New session dialog to add cards to
  it, e.g. from another computer. Images of the folder that the session already has, matched by file name, are
  skipped.
- **Sell on eBay.** List a session's cards on eBay with your own eBay developer keys: each card becomes a draft with
  its pictures, title, item specifics and condition (graded slabs with their grader and grade), at Ximilar's price
  or yours. Publish the drafts when they look right; Cardloader then shows which listings sold. See
  [Selling on eBay](#selling-on-ebay).
- **TCGplayer, Whatnot and Shopify.** Export a session's ungraded trading cards as a TCGplayer list (with each card's
  TCGplayer product link), as Whatnot's bulk listing CSV for auctions and Buy it Now, or as Shopify products with
  condition variants (or a Shopify inventory update), with the pictures uploaded to ImgBB. See
  [TCGplayer, Whatnot and Shopify](#tcgplayer-whatnot-and-shopify).
- **CSV import and export.** Import a spreadsheet as a new session: rows with a card name come in as identified
  cards, rows with only an image path are analysed. Any session can be exported to CSV, and an export imports
  back as it was.
- **Manage sessions.** Search and filter them, rename them (the Cards Cloud copy follows), change their settings,
  duplicate them to analyse the same source again, export the cards to CSV, and delete one or several at once,
  optionally with their Cards Cloud copies. Remote sessions can be renamed and deleted from the Cards Cloud tab.
- **Review.** Every identified card is to review until you accept or correct it. Filter a session to the cards to
  review (sort by Match to see the unsure ones first), failed or corrected
  ones, or search it. Accept a match with **A** (cards to review show an amber question mark, accepted ones a green check; **A** again, or **Unaccept**, takes it back), pick a candidate with **1–9**, edit a card's name, set, number,
  price, location or notes with **E**, and move with **J**/**K**; press **?** for all shortcuts. Tick cards (Shift
  for a range) to accept, analyse again, set where they are kept or delete them at once. Corrections are sent to
  Cards Cloud too. A notification tells you when a session has finished while you were in another app.
- **Home.** Your cards, their total value, the cards to review and the API calls of the last 7 days; a list of
  what needs attention (sessions to review, failed cards, interrupted sessions to resume, upload errors); and a year
  of activity. **New session** estimates the API calls before you start.
- **Cards.** Every identified card of your sessions in one list: search by name, set, number, location or notes,
  filter to cards to review, graded cards or duplicates (×3 shows you have three), with the total value. Tick cards
  to set where they are kept (a box, a binder page). When listing or exporting, **Adjust prices** sets the chosen
  cards to a percentage of the market price, ending in .99, with a minimum.
- **Upside-down cards.** When Ximilar finds a card upside down in its image, the card shows an **Upside down**
  badge; click it and **Turn upright** to turn the picture the app shows and exports upload (and the scan, for
  scanner sessions). Images in your own folders are never changed.
- **AI agents.** Claude Desktop, Claude Code, Cursor and other AI apps can work with Cardloader over MCP: start a
  session on a folder, wait for it, and review its cards by looking at their pictures. Cards an agent accepts get an
  AI mark until you confirm them. See [AI agents](#ai-agents-mcp).
- **Pause, stop, resume.** You can pause, stop and resume a session, even after the app restarts. Failed cards can
  be retried individually. When the connection drops, the session pauses instead of failing its cards.

## Install

Download the installer for your platform:

| Platform | Download |
| --- | --- |
| macOS 11+ (Apple silicon + Intel) | [Cardloader .dmg][dl-dmg] |
| Windows 10/11 | [Installer (.exe)][dl-exe] or [.msi][dl-msi] |
| Linux | [.AppImage][dl-appimage], [.deb][dl-deb] or [.rpm][dl-rpm] |

[dl-dmg]: https://github.com/Ximilar-com/cardloader/releases/download/v0.3.0/Cardloader_0.3.0_universal.dmg
[dl-exe]: https://github.com/Ximilar-com/cardloader/releases/download/v0.3.0/Cardloader_0.3.0_x64-setup.exe
[dl-msi]: https://github.com/Ximilar-com/cardloader/releases/download/v0.3.0/Cardloader_0.3.0_x64_en-US.msi
[dl-appimage]: https://github.com/Ximilar-com/cardloader/releases/download/v0.3.0/Cardloader_0.3.0_amd64.AppImage
[dl-deb]: https://github.com/Ximilar-com/cardloader/releases/download/v0.3.0/Cardloader_0.3.0_amd64.deb
[dl-rpm]: https://github.com/Ximilar-com/cardloader/releases/download/v0.3.0/Cardloader-0.3.0-1.x86_64.rpm

Release notes and earlier versions are on the [Releases](https://github.com/Ximilar-com/cardloader/releases) page.

> Early builds may be unsigned. On macOS, the first launch says Apple could not verify Cardloader: click **Done**,
> then **System Settings → Privacy & Security → Open Anyway**. On Windows, choose **More info → Run anyway** in
> SmartScreen.

To build and install from source instead, see [Development](#development) and run `just install`.

## Getting started

1. On first launch, answer the setup questions (or skip them); they set the defaults of new sessions.
2. Open **Settings**, paste your Ximilar API token and click **Test connection**. You can find the token in the
   [Ximilar App](https://app.ximilar.com). Collectibles recognition needs the Business plan or a trial;
   Cards Cloud sync needs the Cards Cloud service on your account. The token is stored in the system keychain.
3. Click **New session** (on Home or Sessions). Pick a folder (or a scanner), choose how to identify the cards, whether to
   grade them and whether to sync to Cards Cloud, then click **Create & run**.
4. Watch the cards come in. Click any row to see its details. Use **Pause** and **Stop** whenever you like;
   **Run again** picks up where the session left off, including new files added to the folder.

To bring in cards you already have in a spreadsheet, click **Import CSV** instead (see below).

Every analysed card costs Ximilar API credits: one call per recognition (two in Auto mode), plus one per
grading. Cards Cloud bills one call per session and one per stored image. See [Ximilar pricing](https://www.ximilar.com/pricing/).

## Local model

**Settings → Identification → Local model** switches new sessions to on-device identification. Pick a model,
accept its license and download it (with progress; downloads can be cancelled and resumed). Each session keeps
the engine and model it was created with, and **New session** can pick another downloaded model per session.

| Model | Download | Memory | Time per card |
| --- | --- | --- | --- |
| [LFM2.5-VL 3B](https://huggingface.co/LiquidAI/LFM2.5-VL-3B-GGUF) (default) | 2.3 GB | ~2.5 GB | 3.2 s |
| [LFM2.5-VL 450M](https://huggingface.co/LiquidAI/LFM2.5-VL-450M-GGUF) | 330 MB | ~0.6 GB | 0.7 s |
| [LFM2.5-VL 1.6B](https://huggingface.co/LiquidAI/LFM2.5-VL-1.6B-GGUF) | 1.3 GB | ~1.5 GB | 1.9 s |

Times are for an Apple-silicon Mac (Metal); the CPU on Windows and Linux is slower. To choose, run **Compare with
local model** on your own cards (below).

- The model runs in the app through [llama.cpp](https://github.com/ggml-org/llama.cpp): Metal on Macs, the CPU on
  Windows and Linux. The model is loaded once (a few seconds) and reads one card at a time.
- Two quick looks per card: the whole card for the name, year and game, and the enlarged bottom edge for the
  collector number and set code. The answers are held to a fixed JSON shape, so they always parse.
- What it can't do: tell reprints and variants apart, give a match score, other candidates or prices.
  Rows show a **Local** tag instead of High/Medium/Low. Cards Cloud sync still works (with your Ximilar token).
- **Grading and condition (estimates).** A local session can also grade: each side gets one more question, and
  the card takes the worse side. **Condition** picks a label of the chosen scale (TCGplayer, Cardmarket, eBay
  trading cards or eBay sports cards; Settings → New sessions sets the default, the New session dialog per session).
  **Full grade** asks for corners, edges, surface and centering from 1 to 10 and caps the final grade one point
  above the weakest. Both are rough estimates of a small model, not a grader's opinion.
- **How good is it on your cards?** On a session identified by Ximilar, **⋯ → Compare with local model…** runs
  a downloaded model on the same cards and shows how often the name and the number agree, and how long each card
  took. Run it again with another model to compare models side by side. The session itself isn't changed.
- The models are made by Liquid AI and licensed under the [LFM Open License v1.0](https://www.liquid.ai/lfm-license),
  which limits commercial use above a revenue threshold. It is downloaded from Hugging Face, never bundled.
- Models are stored in the app's data folder under `models/`; **Delete** in Settings removes them.

## Selling on eBay

Cardloader lists cards through eBay's Sell APIs with **your own eBay developer keys**: an open-source app can't
ship eBay's client secret, and nothing runs on a server in between.

1. Join the [eBay developer program](https://developer.ebay.com/) and create a keyset (Sandbox to try it out,
   Production for real listings). eBay activates a Production keyset once you either subscribe to Marketplace
   Account Deletion notifications or apply for the exemption (Cardloader keeps no eBay user data on a server).
2. Under **User Tokens** of the keyset, add an eBay Redirect URL (RuName). Leave its "auth accepted" URL as eBay's
   own page.
3. In **Settings → eBay**, choose the environment and enter the App ID, Cert ID and RuName, then **Connect eBay
   account** and agree on eBay's page. The login lasts about 18 months and is kept in the system keychain.
4. Choose the shipping, payment and return policies and where cards ship from. Accounts without business policies
   can turn them on there; create the policies in eBay's Seller Hub.
5. On a finished session, **⋯ → List on eBay…** shows each identified card with its title, category and condition.
   Prices come from Ximilar; cards without one need a price. **Create drafts** uploads the pictures and creates
   unpublished listings, which you can check on eBay.
6. **Publish on eBay** makes the drafts live. The session shows how many are drafts, listed and sold; listings are
   checked again when the session is opened (at most every 10 minutes) or with the refresh button.

Cards go to eBay US: trading cards to *CCG Individual Cards*, sports cards to *Sports Trading Card Singles*. A card
in a graded slab is listed as Graded with its grader, grade and certificate number; other cards as Ungraded in the
condition chosen in the dialog (Ximilar's own grade is an estimate, not a professional grade). Comics aren't listed
yet. Listing fees and selling limits are eBay's.

## TCGplayer, Whatnot and Shopify

A session's **⋯** menu has **Export for TCGplayer…**, **Export for Whatnot…** and **Export for Shopify…**. Both show the cards with their
prices (from Ximilar; cards without one need a price) and a condition for ungraded cards whose condition wasn't
graded.

- **TCGplayer**: a CSV of the ungraded trading cards in TCGplayer's terms — product line, set, name, number,
  condition (Near Mint, Lightly Played, …), quantity and price, plus the TCGplayer product id and link from the match.
  TCGplayer doesn't give API access to new apps, and its Seller Portal import takes only its own export files (they
  carry its SKU ids), so add the cards in Seller Portal from this list. Graded cards are sold there only as Listings
  with Photos, and TCGplayer has no sports cards; both are left out.
- **Whatnot**: Whatnot's bulk listing CSV (its US template), for Seller Hub → Inventory → **Import from CSV** (draft
  listings) or a show's **Upload CSV**. Choose Auction (the price is the starting bid) or Buy it Now, and a shipping
  profile. Trading cards, sports singles and entertainment cards get Whatnot's categories and conditions; slabs are
  listed as Graded with the grader, grade and certificate in the title and description. Whatnot downloads pictures
  from public links, so with an [ImgBB](https://api.imgbb.com/) API key (Settings → Selling) the export uploads each
  card's front and back there first, once per card. Without a key, add the photos in Whatnot.
- **Shopify**: Shopify's product CSV (Products → **Import**) or its inventory CSV (Products → Inventory →
  **Import**).
  - *Products*: either one product per card (name, set and number), its conditions (Near Mint, Lightly Played, …) as
    variants with the number of copies and the highest of their prices, or one product per scanned card. A graded slab
    is always its own product with the grader and grade in its title. Products get Shopify's trading card category,
    tags (game, set, year, grader), a description, and the pictures (uploaded to ImgBB, as for Whatnot). They are
    drafts unless you choose to publish. Handles are made from the card, so importing again with **Overwrite products
    with matching handles** updates the same products.
  - *Inventory update*: sets each product's stock at a location (named exactly as in Shopify) to the number of
    copies — for stores with several locations, or to restock products imported before (choose the same grouping).

## AI agents (MCP)

Cardloader includes an [MCP](https://modelcontextprotocol.io) server, so AI apps can drive it. Turn it on in
**Settings → AI agents**, choose what agents may do, and copy the setup for your app from the same page:

- **Claude Desktop:** Settings → Developer → Edit Config, then add the server and restart Claude:

  ```json
  { "mcpServers": { "cardloader": { "command": "/Applications/Cardloader.app/Contents/MacOS/cardloader", "args": ["mcp"] } } }
  ```

- **Claude Code:** `claude mcp add cardloader -- /Applications/Cardloader.app/Contents/MacOS/cardloader mcp`
- **Other apps** connect over HTTP to the address Settings shows (`http://127.0.0.1:47821/mcp`) with the header
  `Authorization: Bearer <token>`.

The program path differs on Windows and Linux; Settings shows the right one. `cardloader mcp` passes messages
to the open app. On macOS it opens the app if it isn't running; elsewhere, open Cardloader first. The server listens
on 127.0.0.1 only and needs the token.

Then ask, for example:

- “Run a new Cardloader session on ~/Cards/pokemon-en: English Pokémon, fronts only, store it in Ximilar.”
- “Review the latest Cardloader session: check each card against its picture and accept the right ones.”

Agents can always read sessions, cards and their pictures. Everything else needs a switch in Settings:

| Switch | Lets agents |
| --- | --- |
| **Run and review** (on by default) | Create, run, pause and stop sessions, wait for them, accept cards, pick another candidate, edit cards, analyse cards again, sync to Cards Cloud, and export CSV and TCGplayer files |
| **Delete** | Delete cards and sessions, and Cards Cloud sessions |
| **Marketplaces** | Create eBay drafts, publish and remove listings, and export Whatnot and Shopify files (these upload pictures) |
| **Settings** | Change the defaults of new sessions |

No tool reads or changes your API token or other keys, and agents can't start scanner sessions. Everything an agent
does shows up in the app as it happens. Cards it accepted, picked or edited show a robot icon and its note. Use
the session's **By AI** filter, or **By AI** on the Cards page, to check them. Press **Confirm** (or **A**) to make a
review your own.

## CSV import and export

**Export:** a session's **⋯** menu → **Export CSV…** writes one row per card (UTF-8, comma-separated, readable
by Excel): number, status, type, name, full name, set, set code, card number, series, year, category, confidence,
distance, whether you picked the match, price (USD), grade, condition, the graded slab's company, grade and
certificate number, front and back image paths, error.

**Import:** **Sessions → Import CSV** (or the **CSV file** source in **New session**) shows what the file
contains before anything is imported. One row is one card, and columns are recognised by their header, loosely:

| Field | Headers understood (any case, spaces or underscores) |
| --- | --- |
| Front image | `front`, `image`, `image path`, `path`, `file`, `filename`, `photo`, … |
| Back image | `back`, `back path`, `back image` |
| Card | `name` / `card name`, `full name`, `set` / `set name` / `expansion`, `set code`, `number` / `no` / `collector number`, `series`, `year`, `category` / `game`, `type` |
| Values | `price` / `price usd` / `value` / `market price`, `grade`, `condition`, `distance` |

- A row with a card name becomes an identified card right away, with no API call. Tick **Analyse the images
  … again** to send rows that also have an image to the Ximilar API instead.
- A row with only an image path is analysed when the session runs, like an image in a folder.
- Relative image paths are read from the CSV's folder. Separators `,`, `;` and tab are detected; with `;`, a
  decimal comma (`12,50`) is understood.
- **Run again** on an import session analyses image rows that were added to the CSV since.

## Scanner setup

Cardloader scans in **ADF duplex** mode. The defaults are a 66 × 92 mm frame, which is slightly larger than a
standard 63 × 88 mm card, 600 dpi, colour, and **multifeed detection off** (stiff cards trigger false double-feed
errors). Start with small stacks of up to about 10 cards, and don't feed sleeved cards.

- **Settings → Scanner** applies to every scanner session, also when it is continued.
- **Rotation: Upside down** turns each page as it is scanned, for cards fed bottom edge first.
- **Side order: Back, then front** is for cards fed back side up.
- A session that doesn't grade scans at **300 dpi**: identification gets a preview of at most 1600 px, which a
  300 dpi card nearly fills. Grading uses the full 600 dpi scan.

| OS | What to install | How Cardloader talks to it |
| --- | --- | --- |
| **Windows** | [PaperStream IP (TWAIN)](https://www.pfu.ricoh.com/global/scanners/fi/dl/) for the fi-8170 | Bundled 32-bit TWAIN helper (`cardloader-twain`) |
| **macOS** | `brew install sane-backends` | SANE `scanimage` |
| **Linux** | `sudo apt install sane-utils` (or your distro's SANE package); optionally RICOH's `pfufs` driver | SANE `scanimage` |

- The fi-8170 must be connected over **USB**. Its LAN port is only usable through PaperStream on Windows, and the
  scanner doesn't speak eSCL/AirScan.
- To check the scanner outside Cardloader, run `scanimage -L` (macOS/Linux) or open it in PaperStream Capture
  (Windows).
- On Windows, the TWAIN helper runs as a separate process, so a stuck driver never freezes the app. See
  [`crates/cardloader-twain`](crates/cardloader-twain/README.md) for details and diagnostics.
- If your scanner can't be driven directly, use a **Watch folder** session and let your scanning software save
  into that folder.

## Where data lives

| | macOS | Windows | Linux |
| --- | --- | --- | --- |
| Sessions database, thumbnails, scans, grading overlays | `~/Library/Application Support/com.ximilar.cardloader/` | `%APPDATA%\com.ximilar.cardloader\` | `~/.local/share/com.ximilar.cardloader/` |
| Settings (`settings.json`) | same folder | same folder | `~/.config/com.ximilar.cardloader/` |

Original images in folder sessions are never modified or moved. Deleting a session removes only the files that
Cardloader created.

## Development

Prerequisites:

- [Rust](https://rustup.rs). The toolchain is pinned in `rust-toolchain.toml` and installed automatically.
- [Node.js](https://nodejs.org) 22 or newer.
- [just](https://just.systems). Optional, since a `Makefile` with the same targets exists.
- The [Tauri prerequisites](https://v2.tauri.app/start/prerequisites/) for your OS. On Debian/Ubuntu:
  ```sh
  sudo apt install libwebkit2gtk-4.1-dev libappindicator3-dev librsvg2-dev patchelf libssl-dev
  ```
- [CMake](https://cmake.org) and Clang, to compile llama.cpp for the local model (`brew install cmake` on macOS,
  `sudo apt install cmake clang` on Debian/Ubuntu, the Visual Studio C++ tools on Windows). To build without local
  model support, use `cargo build --no-default-features` in `src-tauri/`.

Then:

```sh
just setup     # npm ci + cargo fetch
just dev       # run the app with hot reload
just test      # vitest + cargo test (including an end-to-end run against a mock Ximilar API)
just lint      # eslint, prettier, tsc, rustfmt, clippy
just fmt       # rustfmt + prettier
just build     # installers for this platform → target/release/bundle/
just install   # build and install locally
```

All commands run in the repository root. `npm run dev` on its own serves the UI in a normal browser with **demo data**: the Rust commands are mocked in
`src/lib/demo.ts`. This is handy for working on the interface without the Rust side.

### Layout

```
src/                      React UI (pages/, components/, lib/api.ts = typed Rust commands)
src-tauri/src/
  commands.rs             everything the UI can call
  runner.rs               session runner: ingest → recognise → grade (async) → events
  sync.rs                 Cards Cloud mirroring
  ebay/                   eBay: sign-in, listings (drafts, publish, status), Inventory/Media API client
  market/                 TCGplayer, Whatnot and Shopify CSV exports, ImgBB picture hosting
  ximilar/                HTTP client, recognition, grading, Cards Cloud API
  local/                  on-device models: catalog, downloads, llama.cpp engine, prompts, comparison
  mcp/                    MCP server for AI agents (tools, prompts), and the `cardloader mcp` stdio bridge
  source/                 folder, watch folder, CSV file, scanners (SANE CLI, TWAIN sidecar)
  export.rs               CSV export
  db.rs, migrations/      SQLite session store
crates/cardloader-twain/  Windows TWAIN helper (32-bit sidecar)
scripts/prepare-sidecar.mjs  builds the helper and fetches TWAINDSM.dll (Windows only)
```

### Releases

```sh
just update-version 0.2.0          # or patch / minor / major; make update-version VERSION=0.2.0
just update-version patch --dry-run
```

From an up-to-date, clean `main`, this sets the version in `package.json`, `package-lock.json`, `Cargo.toml` and
`Cargo.lock`, runs lint and tests (`--skip-checks` to skip), commits "Release vX.Y.Z", tags it and pushes both
(`--no-push` to keep them local).

Pushing a `v*` tag runs [`.github/workflows/release.yml`](.github/workflows/release.yml), which builds macOS (universal), Windows and Linux
installers into a draft GitHub release; review it and publish it. Builds are unsigned unless you add these repository secrets:

- **macOS** signing and notarization: `APPLE_CERTIFICATE` (base64 .p12), `APPLE_CERTIFICATE_PASSWORD`,
  `APPLE_SIGNING_IDENTITY`, `APPLE_ID`, `APPLE_PASSWORD` (app-specific password), `APPLE_TEAM_ID`.
- **Windows** Authenticode: `WINDOWS_CERTIFICATE` (base64 .pfx), `WINDOWS_CERTIFICATE_PASSWORD`.

## License

[Apache-2.0](LICENSE). Bundled third-party components are listed in [THIRD_PARTY.md](THIRD_PARTY.md).
