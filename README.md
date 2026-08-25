# Cubicle KV

Minimal persistent, concurrent, multi-client key-value TCP server built in Rust using Tokio to learn how databases handle write-ahead logging (WAL), snapshotting, non-blocking persistence, and state recovery.

---

## Overview

* **Async TCP Server:** Handles concurrent client connections over TCP (`127.0.0.1:8080`) using Tokio's async runtime.
* **Write-Ahead Logging (WAL):** Write operations (`SET`, `DELETE`) are stamped with CRC32 checksums and written to `cubicle.wal` with immediate flushing before mutating the in-memory state.
* **Snapshots:** Uses `im::OrdMap` (an immutable BTree) for zero-cost $O(1)$ snapshots, allowing reads and persistence passes without deep-copying key-value pairs.
* **Background Compaction:** A background task periodically dumps state snapshots to `cubicle.snap.tmp` and atomically replaces `cubicle.snap` while truncating the WAL—all without blocking incoming client requests.
* **Data Integrity & Recovery:** Rebuilds complete state on startup by reading the last valid snapshot and replaying non-corrupted trailing WAL records.
* **Typed Value System:** Parses and stores strongly-typed data structures, including `String`, `Integer` (`i64`), `Float` (`f64`), `Boolean` (`bool`), and nested `List` (`Vec<Value>`).

---

## Command Reference

Commands are sent as plain-text lines over a TCP connection. Keys containing spaces can be enclosed in double quotes (e.g., `"my key"`).

| Command | Usage | Description | Example Server Response |
| :--- | :--- | :--- | :--- |
| `SET` | `SET <key> <value>` | Save or update a typed key-value pair | `-> OK` |
| `GET` | `GET <key>` | Retrieve value by key | `-> "hello"` or `-> Key not found` |
| `DELETE` | `DELETE <key>` | Remove key from the engine | `-> Deleted` or `-> Key not found` |
| `SEE` | `SEE` | List all key-value pairs in lexicographical order | `key1: "value1"`<br>`key2: 42` |
| `SNAPSHOT` | `SNAPSHOT` | Explicitly trigger a snapshot dump and WAL truncation | `-> Snapshot saved` |

---

## Quick Start

Start the TCP server listener:
```bash
cargo run

# Output
Restored 0 items from disk
Server running on 127.0.0.1:8080
```

Connect using `nc` (netcat), `telnet`, or any TCP socket client:
```bash
nc 127.0.0.1 8080
```

Usage:
```text
SET user:1 "Alice"
-> OK

SET user:1:age 30
-> OK

SET user:1:tags ["rust", "database", true]
-> OK

GET user:1
-> "Alice"

SEE
user:1: "Alice"
user:1:age: 30
user:1:tags: ["rust", "database", true]

SNAPSHOT
-> Snapshot saved

DELETE user:1
-> Deleted
```

## Recovery

If the server crashes or restarts, state restoration happens in two stages:
- **Snapshot Loading:** `cubicle.snap` is read to rebuild the baseline in-memory `OrdMap`.
- **WAL Replay:** `cubicle.wal` is read sequentially. Entries with valid CRC32 checksums are re-applied; execution stops cleanly if log corruption is encountered.
