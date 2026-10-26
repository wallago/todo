# README

## Design

```
todo/
├── Cargo.toml            # [workspace] members = ["crates/*"]
├── crates/
│   ├── core/             # Todo struct, state machine, no I/O
│   ├── md/               # parser + serializer (md <-> core)
│   ├── store/            # file watching, read/write vault, index
│   ├── server/           # axum HTTP API + MCP at /mcp — sole owner of the vault
│   ├── client/           # wire types + request fns (shared by tui/desk)
│   └── tui/              # ratatui client
└── vault/                # test fixtures, sample .md
```

```
vault/*.md  <--(watch + read/write)-->  server  <--HTTP-->  tui
                                           ^
                                           +---HTTP--> desk display
                                           +---HTTP /mcp--> Claude
```

### Keys

- `core` has zero dependencies on I/O.
  Everything else depends on it.
  This is the part that keeps the workspace from rotting.
- Keep `md` separate from `store`.
  The parser should be a pure `&str -> Vec<Todo> + Vec<Todo> -> String`, which makes it trivially testable and round-trippable.
  Round-trip tests matter a lot here: a parser that loses a stray comment or reorders lines will eventually corrupt an Obsidian note you care about.
  Write `parse(write(parse(s))) == parse(s)` as a property test early.
- Markdown format: Obsidian Tasks plugin convention as target: `- [ ] buy milk 📅 2026-10-30 #errand` with stable id — `^abc123`.
- MCP: use the official `rmcp` crate.
  Expose maybe 5 tools: `list_todos`, `add_todo`, `complete_todo`, `update_todo`, `search`.
  Resist exposing a generic "edit file" tool — the whole point is that the parser enforces format validity.
- `server` is the only process that touches the vault.
  `store` is server-internal; the MCP tools live in the server and call the same code as the routes.
  Two independent readers/writers means torn reads mid-write and lost updates when Obsidian saves at the same time.
- `client` holds the wire format once.
  The server and `tui` both depend on it, so the API types can't drift between consumers.
- The file watcher must distinguish the server's own writes from external edits.
  Otherwise every write triggers a reload that can clobber in-flight state.
  Record path + mtime/hash just written, ignore the next matching event.
- Incoming Syncthing/Obsidian Sync changes are just external edits.
  Same code path as editing the vault in Obsidian on another device.

### Deployment

- Server runs on the always-on box that holds the vault, as a systemd user service.
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

### Build order

1. `core` + `md` with round-trip tests
2. `store`
3. `server` + `client`
4. `tui` — the thing used daily
5. `mcp`
6. desk display endpoint

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

```sh
claude mcp add --transport http todo https://todo.wallago.xyz/mcp
```

The endpoint has no auth yet, like the API.
