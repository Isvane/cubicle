# Cubicle KV

Minimal persistent, concurrent, multi-client key-value TCP server built in Rust using Tokio to learn how databases handle write-ahead logging (WAL), snapshotting, non-blocking persistence, and state recovery.

---

## Commands

| Command | Usage | Description | RESP Response Type | Example Raw Output |
|:---|:---|:---|:---|:---|
| `SET` | `SET <key> <value>` | Save or update a typed key-value pair | Simple String | `+OK\r\n` |
| `GET` | `GET <key>` | Retrieve value by key | Bulk String / Null | `$5\r\nAlice\r\n` or `_\r\n` |
| `DEL` / `DELETE` | `DEL <key>` | Remove key from the engine | Integer | `:1\r\n` (found) or `:0\r\n` (not found) |
| `SEE` / `KEYS` | `SEE` | List all key-value pairs | Array / Simple String | `*2\r\n$12\r\nuser:1: Alice\r\n...` |
| `SNAPSHOT` / `SAVE` | `SNAPSHOT` | Explicitly trigger snapshot dump and WAL truncation | Simple String | `+OK\r\n` |

---

## Quick Start

Start the TCP server listener:
```bash
cargo run

# Output
Restored 0 items from disk
Server running on 127.0.0.1:8080
```

Connect using `redis-cli`:
```bash
redis-cli -p 8080
```

Usage:
```text
127.0.0.1:8080> SET user:1 "Alice"
OK

127.0.0.1:8080> SET user:1:age 30
OK

127.0.0.1:8080> GET user:1
"Alice"

127.0.0.1:8080> DEL user:1
(integer) 1

127.0.0.1:8080> SEE
1) "user:1:age: 30"
```

## Recovery

If the server crashes or restarts, state restoration happens in two stages:
- **Snapshot Loading:** `cubicle.snap` is read to rebuild the baseline in-memory `OrdMap`.
- **WAL Replay:** `cubicle.wal` is read sequentially. Entries with valid CRC32 checksums are re-applied; execution stops cleanly if log corruption is encountered.
