# Contributing to Cardloader

Thanks for helping! Issues and pull requests are welcome. Please report security issues privately, as described in
[SECURITY.md](SECURITY.md).

1. Set up the toolchain as described in the [README](README.md#development) and run `just setup`.
2. Make your change. Keep the style of the surrounding code: `just fmt` formats Rust (`rustfmt`) and the frontend
   (Prettier); `eslint` and `tsc` check the TypeScript.
3. Run `just lint` and `just test`. CI runs the same checks on Windows, macOS and Linux.
4. Open a pull request describing what changed and how you tested it. For scanner changes, say which scanner,
   driver and OS you tried.

## Tips

- `npm run dev` alone serves the UI with demo data in a normal browser (`src/lib/demo.ts`).
- Rust commands live in `src-tauri/src/commands.rs`; their TypeScript types are in `src/lib/api.ts` — update both.
- Database schema changes go in a new file in `src-tauri/migrations/` (append to `MIGRATIONS` in `db.rs`); never
  edit a released migration.
- The runner's end-to-end tests (`src-tauri/src/runner.rs`) run a whole session against a mock Ximilar API with
  `wiremock`; extend them when you change the pipeline.
- Don't commit API tokens, real customer data or personal paths. Use your own Ximilar token for manual testing; it
  lives in the system keychain, never in the repository.
