# Daemon protocol, version 1

The wally daemon listens on a Unix socket so that the menu bar app and the CLI
can drive it. The daemon is the only process that sets wallpapers and the only
one that writes `config.yaml` and `marks.yaml` while it runs.

## Transport

- Socket: `$XDG_STATE_HOME/wally/wally.sock`, by default
  `~/.local/state/wally/wally.sock`, mode `0600`, removed and recreated when
  the daemon starts.
- One JSON object per line, UTF-8, `\n`-terminated, both ways.
- A client may send several requests on one connection; responses come back in
  order. Every request carries an `id` (integer) echoed in its response.

```json
{"id": 1, "cmd": "next", "screen": 2}
{"id": 1, "ok": true, "result": {...}}
{"id": 1, "ok": false, "error": "no screen with id 9"}
```

- Unknown fields are ignored; an unknown `cmd` answers `ok: false`.
- A client starts with `hello` and refuses a daemon whose `protocol` differs
  from its own.

## Shared shapes

`Screen`:

```json
{"index": 1, "id": 69733382, "name": "DELL U2723QE",
 "image": "/Users/me/Pictures/Wallpapers/forest.jpg",
 "favorite": false}
```

`index` is the 1-based position, left to right, as in `wally show`. `image` is
what macOS reports for the screen (may be `null`).

`Status`:

```json
{"version": "0.0.2", "protocol": 1, "pid": 1234, "paused": false,
 "interval": "30m", "interval_secs": 1800,
 "order": "random", "screens_mode": "distinct",
 "recursive": false, "sources": ["~/Pictures/Wallpapers"],
 "images": 1722, "favorites": 12, "banned": 3,
 "last_change": "2026-11-15T10:00:00Z", "next_change": "2026-11-15T10:30:00Z",
 "screens": [Screen, ...],
 "error": null}
```

`next_change` is `null` while paused. `error` carries the last error of the
daemon loop (such as "no image in ..."), `null` when all is well. Timestamps
are RFC 3339, UTC, seconds.

`HistoryEntry`:

```json
{"at": "2026-11-15T10:00:00Z", "screen_id": 69733382,
 "screen_name": "DELL U2723QE", "image": "/path/forest.jpg"}
```

`LibraryItem`:

```json
{"path": "/path/forest.jpg", "name": "forest.jpg", "favorite": true}
```

Banned images never appear in `library`.

`Config` is `config.yaml` as JSON:

```json
{"sources": ["~/Pictures/Wallpapers"], "recursive": false,
 "interval": "30m", "order": "random", "screens": "distinct"}
```

## Commands

| `cmd` | Arguments | `result` |
|---|---|---|
| `hello` | | `{"version": "0.0.2", "protocol": 1}` |
| `status` | | `Status` |
| `next` | `screen` (id, optional: all screens) | `Status` after the change |
| `set` | `screen` (id), `image` (path) | `Status` after the change |
| `pause` | | `Status` |
| `resume` | | `Status` (the interval restarts from now) |
| `favorite` | `image` (path, optional: the image of `screen`), `screen` (id, optional), `on` (bool, default `true`) | `Status` |
| `ban` | same as `favorite` | `Status`; banning an image on screen replaces it right away |
| `history` | `limit` (default 50, newest first) | `[HistoryEntry]` |
| `library` | `query` (optional, words as in `wally search`) | `[LibraryItem]` |
| `config_get` | | `Config` |
| `config_set` | `key` (`interval`, `order`, `screens`, `recursive`), `value` (string, same values as `wally settings set`, shortcuts included) | `Config` |
| `source_add` | `folder` (path, `~` allowed) | `Config` |
| `source_remove` | `folder` | `Config` |
| `subscribe` | | `Status`, then events on the same connection |

After `subscribe`, the daemon pushes, without `id`:

```json
{"event": "status", "status": Status}
```

whenever the status changes: a change of image (scheduled, `next`, `set`, a
Space catching up), pause or resume, a configuration or marks change, a screen
plugged or unplugged, a new or cleared error. A subscribed connection accepts
no further request; a client opens a second connection for its commands.

## Files

| File | Owner | Content |
|---|---|---|
| `~/.config/wally/config.yaml` | daemon (CLI when the daemon is off) | sources, interval, order, screens, recursive |
| `~/.config/wally/marks.yaml` | daemon (CLI when the daemon is off) | `favorites:` and `banned:`, lists of absolute paths |
| `~/.local/state/wally/state.json` | daemon and CLI | deck, cursor, last change, image per screen, `paused` |
| `~/.local/state/wally/history.jsonl` | daemon and CLI | one `HistoryEntry` per line, the last 500 kept |

Favorites are drawn three times as often as other images in random order, and
once per lap in sequential order.
