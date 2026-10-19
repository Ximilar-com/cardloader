# cardloader-twain

A small Windows-only helper process ("sidecar") that lets the cardloader
desktop app scan trading cards in batches through **TWAIN**. It was written
for the RICOH/Fujitsu **fi-8170** with the PaperStream IP (TWAIN) driver, and
should work with any TWAIN 2.x scanner that has an automatic document feeder.

The app starts the sidecar, writes JSON commands to its stdin and reads
JSON events from its stdout. Scanned pages are written as PNG files to a
directory chosen by the app.

On macOS and Linux the binary still builds, so `cargo build --workspace`
works in CI. Running it there only prints
`{"event":"error","code":"unsupported_platform",...}` and exits with code 1.

## Why a separate 32-bit process?

A TWAIN data source (the driver) is a DLL that gets loaded **into the
process that uses it**, so the process must have the same bitness as the
driver. A default PaperStream IP install provides only a **32-bit** TWAIN
driver, while the Tauri app is 64-bit. So the app ships this sidecar built for
`i686-pc-windows-msvc`:

```sh
cargo build --release -p cardloader-twain --target i686-pc-windows-msvc
```

A 64-bit build (`x86_64-pc-windows-msvc`) also works, but only with 64-bit
TWAIN drivers. The `ready` event reports which one is running (`"arch":"x86"`
or `"x86_64"`).

Running it in its own process has another benefit: if the driver crashes, it
can't bring the app down with it.

## Finding the TWAIN DSM

The sidecar looks for the Data Source Manager in this order:

1. `TWAINDSM.dll` in the same directory as `cardloader-twain.exe`. **This is
   the recommended setup.** Bundle the 32-bit `TWAINDSM.dll` 2.5.x from
   <https://github.com/twain/twain-dsm/releases> next to the exe. The DSM is
   licensed under LGPL-2.1, so ship it as a separate, unmodified DLL and
   include its license notice.
2. `TWAINDSM.dll` on the normal DLL search path. Many drivers install it into
   `C:\Windows\SysWOW64` (32-bit) or `System32` (64-bit).
3. 32-bit builds only: the old TWAIN 1.x `twain_32.dll` that ships with
   Windows.

## Protocol

Each line is one JSON object. stdout carries **only** protocol events and is
flushed after every line. Logs go to stderr.

### Commands (stdin)

| command | fields |
|---|---|
| `{"cmd":"list"}` | — |
| `{"cmd":"probe","device":"fi-8170"}` | `device` optional |
| `{"cmd":"scan", ...}` | see below |
| `{"cmd":"stop"}` | stops the feeder and finishes the pages already in the scanner |
| `{"cmd":"configure","device":"fi-8170","settings_path":"C:\\...\\fi8170.twcd"}` | opens the driver's own settings dialog |
| `{"cmd":"quit"}` | stops any scan, closes TWAIN and exits; EOF on stdin does the same |

Fields of `scan`:

```json
{"cmd":"scan","device":"fi-8170","out_dir":"C:\\scans\\batch1","dpi":300,
 "color":"color","duplex":true,"page_width_mm":66.0,"page_height_mm":92.0,
 "multifeed":"off","prefix":"scan","start_index":1,"settings_path":null}
```

- `device`: a product name, or a case-insensitive substring of one. When it
  is left out, the sidecar uses the first source whose name contains `fi-8`,
  and otherwise the system's default TWAIN source.
- `color`: `color`, `gray` or `bw`.
- `multifeed`: `off` turns double-feed detection off, because thick cards
  often trigger ultrasonic sensors. `default` leaves the driver's setting
  unchanged.
- `page_width_mm` / `page_height_mm`: when both are given, the scan area is
  set to exactly this size, starting at the top-left corner.
- `settings_path`: a driver settings file saved earlier by `configure`. It is
  applied *before* the explicit parameters, so the explicit parameters win.
- Only `out_dir` is required. The defaults are 300 dpi, color, duplex,
  multifeed `default`, prefix `scan` and start index 1.

### Events (stdout)

```json
{"event":"ready","version":"0.1.0","arch":"x86"}
{"event":"devices","devices":[{"id":"fi-8170dj","name":"PFU fi-8170dj","backend":"twain"}]}
{"event":"probe","source":{...identity...},"ds2":true,"caps":{"CAP_DUPLEXENABLED":{...},...}}
{"event":"status","message":"scanning"}
{"event":"cap","name":"ICAP_XRESOLUTION","requested":300,"actual":300.0,"ok":true}
{"event":"page","path":"C:\\scans\\batch1\\scan_0001.png","index":1,"side":"front"}
{"event":"error","code":"paper_jam","message":"...","fatal":true}
{"event":"done","pages":42}
{"event":"configured","settings_path":"...","saved":true,"cancelled":false}
```

How each command ends:

- `list` always ends with `devices`. The list is empty if an error happened.
- `probe` ends with either `probe` or an `error` that has `fatal: true`.
- `scan` always ends with `done`, even when it fails, and `pages` is the
  number of files written. If the feeder is empty you get
  `error` `no_media` (with `fatal: false`) followed by `done` with
  `{"pages":0}`.
- `configure` ends with `configured`, or with an `error` that has
  `fatal: true`.
- A command sent while a scan or configure session is running is rejected
  with `error` `busy` (`fatal: false`). That error refers only to the
  rejected command. The running session continues.

Error codes: `paper_jam`, `double_feed`, `no_media`, `no_device`,
`device_offline`, `twain_error`, `dsm_not_found`, `custom_ds_data_unsupported`,
`io`, `bad_command`, `busy`, `cancelled`, `unsupported_platform`. TWAIN
failures are translated from the source's condition code (`TWCC_*`), and the
raw code is included in `message`. `fatal: true` means the current command
was aborted.

Every PNG is written as `<prefix>_<NNNN>.png.tmp` first and then renamed, so a
file watcher never sees a partly written image. Each PNG records the scan
resolution (pHYs).

`side` comes from the extended image info `TWEI_PAGESIDE` when the driver
supports it. Otherwise the sidecar guesses: in duplex mode it alternates
front/back, and in simplex mode every page is a front. The guess is wrong if
the driver skips blank pages.

## How it works

- **Threads.** The main thread is the TWAIN thread. It initialises COM as an
  STA, owns a hidden top-level window (some drivers misbehave with a
  message-only window), and runs a `GetMessage` loop. A separate stdin thread
  parses commands, passes them over a channel and wakes the main thread with
  `PostMessage(WM_APP+1)`. Every `DSM_Entry` call runs on the main thread.
  `stop` also sets an atomic flag, which the transfer loop checks between
  pages.
- **Events from the driver.** If the DSM and the source both support TWAIN 2
  (`DF_DSM2` and `DF_DS2`), the sidecar registers `DAT_CALLBACK2`. The
  callback only posts `WM_APP+2` and never calls back into the DSM. Otherwise
  every message is first passed to the source through `MSG_PROCESSEVENT`
  while the source is enabled (state 5 or higher).
- **Memory.** Containers use the DSM allocator from `DAT_ENTRYPOINT`, or
  `Global*` with a TWAIN 1.x DSM. Native image transfers return a global DIB.
  It supports 1, 4, 8, 24 and 32 bits per pixel, palettes, and bottom-up or
  top-down row order, and is converted to 8-bit gray or RGB PNG.
- **Scan sequence.** `OPENDSM` → choose a source → `OPENDS` → optional
  callback → optional `DAT_CUSTOMDSDATA` replay → capabilities (each one is
  set, read back with `MSG_GETCURRENT` and reported in a `cap` event) →
  check `CAP_FEEDERLOADED` → `ENABLEDS` with the driver UI hidden →
  `MSG_XFERREADY` → a loop of `IMAGEINFO` → `IMAGENATIVEXFER` →
  `EXTIMAGEINFO` → PNG → `ENDXFER`, until no transfers are pending
  (`MSG_STOPFEEDER` once if `stop` was requested) → `DISABLEDS` → `CLOSEDS`
  → `CLOSEDSM`.

## Diagnosing a scanner with `probe`

```sh
echo {"cmd":"probe"} | cardloader-twain.exe
```

Or keep the process open and type `{"cmd":"list"}`, then
`{"cmd":"probe","device":"fi-8170"}`.

The `probe` event contains:

- the source identity: manufacturer, product family and name, driver
  version, TWAIN protocol, supported groups, and whether callbacks are used;
- `CAP_SUPPORTEDCAPS`, as raw ids and as names;
- for each capability that matters here (duplex, feeder, transfer count,
  indicators, UI control, custom DS data, double-feed detection, automatic
  border detection and deskew, pixel type, bit depth, resolution, supported
  sizes, transfer mechanism, autofeed, feeder loaded): the `MSG_GET`
  container decoded as JSON (`onevalue`, `enumeration`, `array` or `range`,
  with FIX32 values as numbers) and the operations `MSG_QUERYSUPPORT`
  reports (`get`, `set`, `reset`, ...).

The `cap` events of a real scan show what the driver actually accepted. The
most important line is `"ok":false` on `CAP_DOUBLEFEEDDETECTION`.

## Known risks / open questions

- **Turning double-feed detection off.** PaperStream may ignore
  `CAP_DOUBLEFEEDDETECTION`, or a driver profile may override it. Check the
  `cap` event. If it says `ok:false`, turn multifeed detection off in the
  driver dialog (`configure`) and pass the saved `settings_path` with every
  scan.
- **Support for `DAT_CUSTOMDSDATA`.** Saving and restoring driver settings
  only works if the driver reports `CAP_CUSTOMDSDATA = TRUE`. If it doesn't,
  `configure` reports `custom_ds_data_unsupported` and saves nothing.
- **Modal error dialogs.** On a paper jam or double feed, PaperStream may show
  its own modal dialog even with the UI hidden. The message loop keeps
  running so the dialog works, but the scan blocks until the user answers it.
  The dialog's parent is a hidden window, so it can open behind the app.
- **Driver UI with `MSG_ENABLEDSUIONLY`.** If a driver runs its dialog
  modally inside the call and reports `MSG_CLOSEDSOK` without a callback,
  the notification can get lost. In that case send `stop` to cancel.
- **Values not verified on hardware:** TWEI_PAGESIDE support, the
  `MSG_STOPFEEDER` behaviour, and the `Count` semantics of the fi-8170
  (0xFFFF while the feeder still has paper).
