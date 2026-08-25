use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use crc32fast::hash;
use im::OrdMap;
use tokio::{
    fs::OpenOptions,
    io::{AsyncBufReadExt, AsyncWriteExt, BufReader},
    net::{TcpListener, TcpStream},
    sync::{Mutex, RwLock},
};

mod cubicle;

use cubicle::cmd::{Cmd, Value};
use cubicle::persistence::{WAL, create_snapshot, restore_state};

struct AppState {
    kv: RwLock<OrdMap<String, Value>>,
    wal_file: Mutex<tokio::fs::File>,
    dirty: AtomicBool,
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    let initial_map = restore_state().await;
    println!("Restored {} items from disk", initial_map.len());

    let wal_file = OpenOptions::new().create(true).append(true).open(WAL).await;

    let file = match wal_file {
        Ok(file) => file,
        Err(e) => return Err(e.into()),
    };

    let state = Arc::new(AppState {
        kv: RwLock::new(initial_map),
        wal_file: Mutex::new(file),
        dirty: AtomicBool::new(false),
    });

    let bg_state = Arc::clone(&state);

    tokio::spawn(async move {
        loop {
            tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;

            if bg_state.dirty.swap(false, Ordering::AcqRel) {
                let snapshot_copy = bg_state.kv.read().await.clone();

                if create_snapshot(&snapshot_copy).await.is_ok() {
                    let mut wal_lock = bg_state.wal_file.lock().await;

                    if let Ok(new_wal) = OpenOptions::new()
                        .create(true)
                        .write(true)
                        .truncate(true)
                        .open(WAL)
                        .await
                    {
                        *wal_lock = new_wal;
                    }
                }
            }
        }
    });

    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    println!("Server running on 127.0.0.1:8080");

    loop {
        let (socket, addr) = listener.accept().await?;
        println!("New connection from: {}", addr);

        let state_clone = Arc::clone(&state);

        tokio::spawn(async move {
            handle_client(socket, state_clone).await;
        });
    }
}

async fn handle_client(mut socket: TcpStream, state: Arc<AppState>) {
    let (reader, mut writer) = socket.split();
    let mut bufreader = BufReader::new(reader);
    let mut line = String::new();

    loop {
        line.clear();

        match bufreader.read_line(&mut line).await {
            Ok(0) => break,
            Ok(_) => match line.trim_end().parse::<Cmd<String>>() {
                Ok(cmd) => match cmd {
                    Cmd::Get(key) => {
                        let val = state.kv.read().await.get(&key).cloned();
                        match val {
                            Some(v) => {
                                let _ = writer.write_all(format!("-> {}\n", v).as_bytes()).await;
                            }
                            None => {
                                let _ = writer.write_all(b"-> Key not found\n").await;
                            }
                        }
                    }
                    Cmd::Set(key, val) => {
                        let payload = format!("SET {} {}", key, val);
                        let crc = hash(payload.as_bytes());
                        let log = format!("{:08x} {}\n", crc, payload);

                        let mut wal = state.wal_file.lock().await;
                        if wal.write_all(log.as_bytes()).await.is_ok() && wal.flush().await.is_ok()
                        {
                            state.kv.write().await.insert(key, val);
                            state.dirty.store(true, Ordering::Release);
                            let _ = writer.write_all(b"-> OK\n").await;
                        } else {
                            let _ = writer
                                .write_all(b"-> Error: Failed to write to WAL\n")
                                .await;
                        }
                    }
                    Cmd::Delete(key) => {
                        let payload = format!("DELETE {}", key);
                        let crc = hash(payload.as_bytes());
                        let log = format!("{:08x} {}\n", crc, payload);

                        let mut wal = state.wal_file.lock().await;
                        if wal.write_all(log.as_bytes()).await.is_ok() && wal.flush().await.is_ok()
                        {
                            let removed = state.kv.write().await.remove(&key);
                            if removed.is_some() {
                                state.dirty.store(true, Ordering::Release);
                                let _ = writer.write_all(b"-> Deleted\n").await;
                            } else {
                                let _ = writer.write_all(b"-> Key not found\n").await;
                            }
                        } else {
                            let _ = writer
                                .write_all(b"-> Error: Failed to write to WAL\n")
                                .await;
                        }
                    }
                    Cmd::See => {
                        let snapshot = state.kv.read().await.clone();
                        if !snapshot.is_empty() {
                            for (key, val) in snapshot.iter() {
                                let _ = writer
                                    .write_all(format!("{}: {}\n", key, val).as_bytes())
                                    .await;
                            }
                        } else {
                            let _ = writer.write_all(b"Cubicle is empty\n").await;
                        }
                    }
                    Cmd::Snapshot => {
                        let snapshot = state.kv.read().await.clone();
                        if let Err(e) = create_snapshot(&snapshot).await {
                            let _ = writer
                                .write_all(
                                    format!("-> Error creating snapshot: {}\n", e).as_bytes(),
                                )
                                .await;
                        } else {
                            let mut wal = state.wal_file.lock().await;
                            if let Ok(new_wal) = OpenOptions::new()
                                .create(true)
                                .write(true)
                                .truncate(true)
                                .open(WAL)
                                .await
                            {
                                *wal = new_wal;
                                state.dirty.store(false, Ordering::Release);
                                let _ = writer.write_all(b"-> Snapshot saved\n").await;
                            } else {
                                let _ = writer.write_all(b"-> Failed to truncate WAL\n").await;
                            }
                        }
                    }
                },
                Err(e) => {
                    let _ = writer
                        .write_all(format!("-> Error: {}\n", e).as_bytes())
                        .await;
                }
            },
            Err(_) => break,
        }
    }
}
