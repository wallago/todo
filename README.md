# README

## Design

```
todo/
├── Cargo.toml            # [workspace] members = apps/server, apps/tui, crates/*
├── justfile              # `just ci`: repo-wide checks, then each app's own
├── apps/                 # one folder per app, each with its own justfile
│   ├── server/           # axum HTTP API + MCP at /mcp — sole owner of the database
│   ├── tui/              # ratatui client, works offline
│   └── web/              # React web app (PWA) served by the server at /, works offline
├── crates/               # libraries the apps share (`just crates` checks them)
│   ├── core/             # Todo struct, state machine, no I/O
│   ├── md/               # read-only Obsidian Tasks parser, for `todo-server import`
│   ├── store/            # SQLite database: topics, todos, vault import
│   └── client/           # wire types + request fns (shared by tui/desk)
└── vault/                # sample Obsidian note, md parser test fixture
```

```
todo.db (SQLite)  <-->  server  <--HTTP-->  tui
                          ^
                          +---HTTP--> web app (phone, browser)
                          +---HTTP--> desk display / phone widget
                          +---HTTP /mcp--> Claude
```

### Keys

- `core` has zero dependencies on I/O.
  Everything else depends on it.
- `server` is the only process that opens the database.
  `store` is server-internal; the MCP tools live in the server and call the same code as the routes.
- `client` holds the wire format once.
  The server and `tui` both depend on it, so the API types can't drift between consumers.

### Checks

`just ci` is the gate, and CI runs each of its recipes as a job in the flake's CI shell
(`nix develop .#ci`): Nix lint, unused deps, `cargo deny`, `cargo audit`, editorconfig,
typos, Markdown links, workflow lint, then format, TOML format, clippy `-D warnings`,
tests and docs for `crates/` and each app (for `apps/web`: biome, `tsc`, vitest and the Vite build).
Commits are checked as Conventional Commits.

### Database

One SQLite file (`[database] path`, created on first start), migrated with `PRAGMA user_version`.

- `topics`: `id` (UUID), `name` (unique, like `projects/todo/main`), `created_at`, `updated_at`, `deleted_at`.
- `todos`: `id`, `topic_id`, `text`, `status`, `priority`, `due`, `done`, `tags` (JSON list),
  `pinned`, `recur` (JSON, like `{"every": 2, "unit": "week"}`), `created_at`, `updated_at`, `deleted_at`.
- Pinned: being worked on, usually with no due date; listed first in the TUI.
- Recurring: completing one adds the next, due one interval after its due date (or after today).
  The done one stops recurring, so reopening it can't add a second copy.
- New ids are UUIDv7 (or the client's own UUID); todos imported from Obsidian keep their `^abc123` block id.
- Topic names are `/`-separated paths without `.md`, like `projects/todo/main`; a trailing `.md` sent by a client is dropped.
- Deleting only sets `deleted_at`, so offline clients can sync it later. Ids are never reused.
- Timestamps are RFC 3339 UTC text, so they sort as text.

### Sync (offline clients)

- Every insert or change takes the next number of one counter (`sync_counter`, bumped by triggers) into the row's `version`.
  No clocks involved, so a device with a wrong clock can't lose changes.
- `GET /api/sync?since=N` answers every topic and todo with `version > N`, deleted ones too (`"deleted": true`),
  and the `cursor` to send next time. `since=0` is the full snapshot.
- Offline writes replay the normal routes. `POST /api/todos` takes a client-made UUID `id`: sending it twice answers
  the saved todo (200) instead of a copy. `DELETE` of a deleted todo answers 204. `complete` was already safe to retry.
- Conflicts: the last write the server receives wins. Fine for one person.

The TUI works offline: it keeps a copy of every todo in `~/.local/share/todo-tui/replica.json`
(one server at a time), and queues each add, edit, tick and delete there. It sends the queue then
pulls `/api/sync` after each change, every 30 s and on `r`. The title bar shows `offline · 3 to send`
until it gets through. A change the server refuses (a todo deleted elsewhere, say) is dropped, shown as
`refused: ...`, and undone by a full resync. Ticking a recurring todo offline: its next one appears once synced.

The web app (`apps/web`) does the same in the browser: the copy and its queue live in `localStorage`,
and a service worker keeps the app itself, so it opens with no network. Install it from the browser
menu ("Add to home screen") to get it as an app on a phone. It asks for the server token once.
todo-server serves it at `/` when `[web] dir` points at the build (`just web::build` writes
`apps/web/dist`; the NixOS module sets it to the `web` package). The page itself needs no token;
its API calls do.

### Import from Obsidian

Once, to move an old vault in (never writes to the vault; safe to run again, known ids are skipped):

```sh
todo-server --config config.toml import ~/vault
# or straight at the file:
todo-server import ~/vault --db /var/lib/todo/todo.db
```

### Deployment

- Server runs on the always-on box that holds the database, as a systemd service.
- Static bearer token on the API, kept in a config file.
  Ten lines of work, and it means exposing over Tailscale later needs no redesign.
  Never bind `0.0.0.0` without it.

### Desk display

- Dedicated endpoint `GET /api/display/today`, not the general API.
  Flat payload of `{text, done}`, already filtered, sorted, and truncated to the lines the screen fits.
  All logic stays server-side in Rust where it's debuggable.
- Serve an `ETag`; device sends `If-None-Match` and skips the redraw on `304`.
  Matters for battery and e-ink panel life.
- Browser device (Pi/tablet): server also serves one static HTML page with a 60s refresh over the same JSON.
  Substantially less work than microcontroller firmware.
- Read-only by default — skips a lot of conflict handling.
  If ticking off from the device is wanted: `POST /api/todos/:id/complete`, idempotent, because the device will retry on flaky wifi.

## MCP

The server serves MCP (Streamable HTTP) at `/mcp`, next to `/api`.
Tools: `list_todos`, `add_todo`, `update_todo`, `complete_todo`, `delete_todo`.
They run the same code as the routes, so there is no second process or port.

`[mcp] allowed_hosts` lists the `Host` names clients may use (loopback by default).
Add the public hostname, or requests get a 403:

```toml
[mcp]
allowed_hosts = ["localhost", "127.0.0.1", "::1", "todo.wallago.xyz"]
```

## Auth

Set `[server] token_file` to a file holding a token (agenix, sops...).
Then `/api` and `/mcp` answer 401 unless the request sends `Authorization: Bearer <token>`.
`/health` stays open.
Without `token_file` there is no auth: keep the server on localhost.

```sh
claude mcp add --transport http todo https://todo.wallago.xyz/mcp \
  --header "Authorization: Bearer $(cat /path/to/token)"
```

In Rust: `todo_client::Client::new(url).with_token(&token)`.

### Read-only token (phone widgets)

`GET /api/agenda` lists open todos due today or overdue, overdue first.
It answers JSON: `date`, `overdue`, `today`, `todos`, and `text` with one line per todo:

```text
⚠ Pay rent (2d late)
• Call the bank
```

Widgets like KWGT can't set headers, so `[server] read_token_file` adds a second token
that `/api/agenda` also takes as `?token=`. It opens nothing else, so a leaked widget can't
change a todo. The main token stays header-only. Use HTTPS: the URL ends up in proxy logs.

```toml
[server]
token_file = "token"
read_token_file = "read-token"
```

NixOS: `services.todo.server.readTokenFile`.

### claude.ai connector (OAuth)

claude.ai custom connectors log in with OAuth, not a fixed header.
Set `[server] public_url` next to `token_file` to turn on a minimal built-in OAuth server:

```toml
[server]
token_file = "token"
public_url = "https://todo.wallago.xyz"
```

Then in claude.ai: Settings > Connectors > Add custom connector, URL `https://todo.wallago.xyz/mcp`.
claude.ai opens a login page on the server: paste the server token once and click Allow.

- `public_url` must be the public HTTPS URL claude.ai reaches (it calls from `160.79.104.0/21`), and its host must be in `[mcp] allowed_hosts`.
- Nothing is stored: codes and tokens are signed with a key derived from the server token.
  A restart keeps everyone logged in; changing the token logs every OAuth client out.
- Access tokens last 1 hour, refresh tokens 90 days.
- The static `Authorization: Bearer <token>` keeps working for the TUI, scripts, and Claude Code.
- Only Claude's redirect URIs are accepted: `https://claude.ai/api/mcp/auth_callback` and loopback URLs.
