use std::sync::{
    Arc,
    atomic::{AtomicBool, Ordering},
};

use bytes::BytesMut;
use crc32fast::hash;
use im::OrdMap;
use tokio::{
    fs::OpenOptions,
    io::{AsyncReadExt, AsyncWriteExt},
    net::{TcpListener, TcpStream},
    sync::{Mutex, RwLock},
};

mod cubicle;

use cubicle::cmd::{Cmd, Value};
use cubicle::persistence::{WAL, create_snapshot, restore_state};
use cubicle::resp::Frame;

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
    let mut buffer = BytesMut::with_capacity(4096);

    loop {
        match socket.read_buf(&mut buffer).await {
            Ok(0) => break,
            Ok(_) => {
                while let Ok(Some(frame)) = Frame::parser(&mut buffer) {
                    let resp_bytes = frame.to_bytes();

                    let response_frame = match Cmd::try_from(frame) {
                        Ok(cmd) => match cmd {
                            Cmd::Get(key) => {
                                let val = state.kv.read().await.get(&key).cloned();
                                match val {
                                    Some(v) => {
                                        Frame::BulkString(Some(v.to_string().as_bytes().to_vec()))
                                    }
                                    None => Frame::Null,
                                }
                            }
                            Cmd::Set(key, val) => {
                                let crc = hash(&resp_bytes);
                                let header = format!("{:08x} ", crc);

                                let mut wal = state.wal_file.lock().await;
                                if wal.write_all(header.as_bytes()).await.is_ok()
                                    && wal.write_all(&resp_bytes).await.is_ok()
                                    && wal.flush().await.is_ok()
                                {
                                    state.kv.write().await.insert(key, val);
                                    state.dirty.store(true, Ordering::Release);
                                    Frame::SimpleString("OK".to_string())
                                } else {
                                    Frame::Error("ERR Failed to write to WAL".to_string())
                                }
                            }
                            Cmd::Delete(key) => {
                                let crc = hash(&resp_bytes);
                                let header = format!("{:08x} ", crc);

                                let mut wal = state.wal_file.lock().await;
                                if wal.write_all(header.as_bytes()).await.is_ok()
                                    && wal.write_all(&resp_bytes).await.is_ok()
                                    && wal.flush().await.is_ok()
                                {
                                    let removed = state.kv.write().await.remove(&key);
                                    if removed.is_some() {
                                        state.dirty.store(true, Ordering::Release);
                                        Frame::Integer(1)
                                    } else {
                                        Frame::Integer(0)
                                    }
                                } else {
                                    Frame::Error("ERR Failed to write to WAL".to_string())
                                }
                            }
                            Cmd::See => {
                                let snapshot = state.kv.read().await.clone();
                                if !snapshot.is_empty() {
                                    let mut elements = Vec::new();
                                    for (k, v) in snapshot.iter() {
                                        let item = format!("{}: {}", k, v);
                                        elements.push(Frame::BulkString(Some(item.into_bytes())));
                                    }
                                    Frame::Array(Some(elements))
                                } else {
                                    Frame::SimpleString("Cubicle is empty".to_string())
                                }
                            }
                            Cmd::Snapshot => {
                                let snapshot = state.kv.read().await.clone();
                                if let Err(e) = create_snapshot(&snapshot).await {
                                    Frame::Error(format!("ERR creating snapshot: {}", e))
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
                                        Frame::SimpleString("OK".to_string())
                                    } else {
                                        Frame::Error("ERR Failed to truncate WAL".to_string())
                                    }
                                }
                            }
                        },
                        Err(e) => Frame::Error(e),
                    };

                    let _ = socket.write_all(&response_frame.to_bytes()).await;
                }
            }
            Err(_) => break,
        }
    }
}
