# Cubicle DB

Minimal key-value store built in Rust to learn how databases store and recover data.

---

## Overview

* **Write-Ahead Logging (WAL):** Write operations (`SET`, `PUT`, `DELETE`) are logged to `cubicle.wal` to preserve state in real time.
* **Snapshotting:** Creates a compact, point-in-time state dump in `cubicle.snap` and truncates the WAL to keep startup fast and log files small.
* **Data Recovery:** Automatically restores state on launch by first loading the snapshot baseline, then replaying remaining WAL entries.
* **In-Memory Storage:** Keeps data in a sorted map for fast lookups and sorted iteration.

---

## Commands

| Command | Usage | Description |
| :--- | :--- | :--- |
| `SET` | `SET <key> <value>` | Save a new key-value pair |
| `GET` | `GET <key>` | Look up a value by its key |
| `PUT` | `PUT <key> <value>` | Update an existing key |
| `DELETE` | `DELETE <key>` | Remove a key |
| `SEE` | `SEE` | Print all stored data |
| `SNAPSHOT` | `SNAPSHOT` | Compact current state to disk and clear the WAL |

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

Close the app and restart it. You will see:
```text
Restored 1 items from disk
```
