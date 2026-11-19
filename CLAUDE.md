# todo

Personal todo system: a server that owns a SQLite database (HTTP API + MCP),
and clients that sync with it, working offline. See `README.md` for the
design.

## Apps

Each app has its own folder, its own `justfile` and its own checks. The root
`justfile` loads each one as a module, and `just ci` runs every app's `ci`
after the repo-wide checks.

- `apps/server/`: its recipes run as `server::<recipe>`.
- `apps/tui/`: its recipes run as `tui::<recipe>`.
- `apps/web/`: Vite + React + TypeScript web app (PWA), pnpm; its recipes run
  as `web::<recipe>`. todo-server serves its `dist/` at `/` from `[web] dir`.

The libraries the apps share live in `crates/` (core, md, store, client);
`just crates` formats, lints, tests and documents them.

## Layout

- `flake.nix`: dev shell (`nix develop`) and the lean CI shell (`nix develop .#ci`). Tools the checks run go in `ciTools`, editor-only ones in `devTools`. Packages, NixOS (`nixosModules.server`) and home-manager (`homeModules.tui`) modules come from `nix/`.
- `crates/core`: the `Todo` type and its rules, no I/O.
- `crates/store`: the SQLite store, its migrations and sync counter; the server is its only user.
- `crates/client`: request/response types and the HTTP client, shared by the server and the TUI.
- `apps/server/src/server/`: axum routes (`todos.rs`, `agenda.rs`), auth, OAuth, and MCP at `/mcp` (`mcp.rs`).
- `apps/tui/src/tui/`: the TUI; `replica.rs` is the offline copy and its queue of changes.
- `apps/web/src/`: the web app; `replica.ts` and `sync.ts` mirror the TUI's offline copy and sync, `store.ts` keeps them in `localStorage`; `sw.ts` is the service worker.
- `nix/web.nix`: builds the web app. Its `pnpmDeps` hash changes with `apps/web/pnpm-lock.yaml`: after a dependency change, set `hash = lib.fakeHash`, build `.#web`, and paste the hash it reports.
- `.github/workflows/ci.yml`: one job per `just ci` recipe, each run in the flake's CI shell, plus the Nix flake check and the commit-format check.

## Rules

Standalone rules live in `.claude/rules/` and are imported here. Only files
reachable from `CLAUDE.md` get loaded, so a new rule needs a line below.

@.claude/rules/nix.md

## Commands

`just` is the entry point, not the raw toolchain; `just --list` shows every
recipe.

- `just ci`: the gate. It runs every check CI runs, recipe by recipe, so a
  green `just ci` is a green CI. Run it before calling a change done.
- `just --list <app>` lists an app's recipes; run them as `<app>::check`,
  `<app>::test`, `<app>::lint`, `<app>::fmt` for the fast loop.

## Constraints

- Clippy runs with `-D warnings` and `Cargo.toml` warns on `unwrap_used`,
  `expect_used`, `panic`, `todo`, `print_stdout` and
  `missing_docs_in_private_items`: no `.unwrap()`, no `.expect()`, no bare
  `panic!`, and every item gets a doc comment.
- The toolchain is nightly (from the flake): `rustfmt.toml` groups imports,
  which stable rustfmt ignores.
- `cargo machete` fails on a dependency nothing uses; drop it in the change that orphans it.
- A new crate has to clear `cargo deny check`: licences, bans, sources and advisories.
- `cargo audit` fails on a dependency with a RustSec advisory.
- `editorconfig-checker` holds every file to `.editorconfig`: indentation, final newline, no trailing spaces.
- `typos` runs over the repo. Add a real term to `typos.toml` rather than rewording around it.
- Every link in Markdown is checked. A placeholder URL fails the gate.
- Commit messages are Conventional Commits (`type(scope): subject`); CI checks every commit of a PR with `committed`, and `git-cliff` builds the changelog from them.

## Verifying a change

`just ci` green is the bar. Say which recipes ran when reporting a change.
The TUI is full-screen: its rendering is covered by `TestBackend` tests, and
anything else gets checked by running it.
