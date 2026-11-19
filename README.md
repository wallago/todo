# README

## Design

```
todo/
├── Cargo.toml            # [workspace] members = ["crates/*"]
├── crates/
│   ├── core/             # Todo struct, state machine, no I/O
│   ├── md/               # read-only Obsidian Tasks parser, for `todo-server import`
│   ├── store/            # SQLite database: notes, todos, vault import
│   ├── server/           # axum HTTP API + MCP at /mcp — sole owner of the database
│   ├── client/           # wire types + request fns (shared by tui/desk)
│   └── tui/              # ratatui client
└── vault/                # sample Obsidian note, md parser test fixture
```

```
todo.db (SQLite)  <-->  server  <--HTTP-->  tui
                          ^
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

### Database

One SQLite file (`[database] path`, created on first start), migrated with `PRAGMA user_version`.

- `notes`: `id` (UUID), `name` (unique, like `projects/todo/main.md`), `created_at`, `updated_at`, `deleted_at`.
- `todos`: `id`, `note_id`, `text`, `status`, `priority`, `due`, `done`, `tags` (JSON list),
  `pinned`, `recur` (JSON, like `{"every": 2, "unit": "week"}`), `created_at`, `updated_at`, `deleted_at`.
- Pinned: being worked on, usually with no due date; listed first in the TUI.
- Recurring: completing one adds the next, due one interval after its due date (or after today).
  The done one stops recurring, so reopening it can't add a second copy.
- New ids are UUIDv7; todos imported from Obsidian keep their `^abc123` block id.
- Deleting only sets `deleted_at`, so offline clients can sync it later. Ids are never reused.
- Timestamps are RFC 3339 UTC text, so they sort as text.

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
