# Cubicle DB

Minimal key-value store built in Rust to learn how databases store and recover data.

---

## Overview

* **File Logging:** Every write command (`SET`, `PUT`, `DELETE`) gets written to a text file (`cubicle.wal`) before updating memory.
* **Data Recovery:** When you start the app, it reads through the log file to rebuild your data automatically.
* **In-Memory Storage:** Stores data in a sorted map, using integer keys and text values.

---

## Commands

| Command | Usage | Description |
| :--- | :--- | :--- |
| `SET` | `SET <key> <value>` | Save a new key-value pair |
| `GET` | `GET <key>` | Look up a value by its key |
| `PUT` | `PUT <key> <value>` | Update an existing key |
| `DELETE` | `DELETE <key>` | Remove a key |
| `SEE` | `SEE` | Print all stored data |

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
```

Close the app and restart it. You will see:
```text
Restored 1 items from WAL
```
