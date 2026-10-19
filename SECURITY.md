# Security policy

## Reporting a vulnerability

Please report security issues privately, not in a public issue: use
[**Report a vulnerability**](https://github.com/Ximilar-com/cardloader/security/advisories/new) on this repository's
Security tab. Describe the issue, how to reproduce it, and the version and operating system you used.

We will confirm the report, keep you posted while we work on a fix, and credit you in the release notes if you wish.

## Supported versions

Fixes go into the latest release only; update to it before reporting.

## Scope

Cardloader runs on your computer and talks to the Ximilar API with your API token. Of particular interest:

- the API token: it is kept in the operating system's keychain (in the settings file when there is none) and sent
  only to the configured API address;
- files the app reads or writes (scans, the session database, CSV import and export) and what the app window can
  load from disk;
- local model downloads, which are pinned to a revision and verified by SHA-256 before use;
- the Windows scanner helper (`cardloader-twain`) and the messages it exchanges with the app.

Problems in the Ximilar API or Cards Cloud themselves are out of scope here; report those to Ximilar support.
