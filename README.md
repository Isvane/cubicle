# Cubicle KV

Minimal persistent key-value store built in Rust to learn how "databases" manage write logs, snapshot state, and recover data cleanly.

---

## Overview

* **Write-Ahead Logging (WAL):** Write operations (`SET`, `PUT`, `DELETE`) are logged to `cubicle.wal` to guarantee durability before updating the in-memory engine.
* **Snapshots:** Uses `im::OrdMap` to take zero-cost, persistent snapshots. Clones cost $O(1)$ by bumping a root reference pointer rather than deep-copying memory.
* **Non-Blocking Background Flushing:** A dedicated background thread takes snapshot dumps to `cubicle.snap` without blocking main-thread CLI execution or holding locks during disk I/O.
* **Data Recovery:** Rebuilds state on startup by combining the baseline snapshot with trailing WAL log replays.
* **Ordered In-Memory Storage:** Maintains deterministic, sorted key iteration (`SEE` command) powered by immutable tree structures.
* **Typed Values:** Strongly-typed value system supporting Strings, Integers (`i64`), Floats (`f64`), Lists (`Vec<Value>`), and Booleans (`bool`).

---

## Commands

| Command | Usage | Description |
| :--- | :--- | :--- |
| `SET` | `SET <key> <value>` | Save or overwrite a key-value pair |
| `GET` | `GET <key>` | Look up a value by key |
| `PUT` | `PUT <key> <value>` | Update an existing key |
| `DELETE` | `DELETE <key>` | Remove a key |
| `SEE` | `SEE` | Print all stored key-value pairs in sorted order |
| `SNAPSHOT` | `SNAPSHOT` | Manually trigger a snapshot dump and clear the WAL |

---

## Quick Start

Run the database:
```text
cargo run
```

Try writing and reading the data:
```text
Enter a command: 
SET 1 hello
-> OK

Enter a command: 
GET 1
-> hello

Enter a command: 
SNAPSHOT
-> Snapshot saved
```

Background auto-saves periodically snapshot dirty states to disk. When restarting the application, your state is automatically restored:
```text
Restored 1 items from disk
```
