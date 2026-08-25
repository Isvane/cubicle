use std::{
    fs::OpenOptions,
    io::{self, Write},
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread,
};

use crc32fast::hash;
use im::OrdMap;

mod cubicle;

use cubicle::cmd::{Cmd, Value};
use cubicle::persistence::{WAL, create_snapshot, restore_state};

fn main() {
    let initial_map: OrdMap<String, Value> = restore_state();
    println!("Restored {} items from disk", initial_map.len());

    let cubicle = Arc::new(RwLock::new(initial_map));
    let dirty = Arc::new(AtomicBool::new(false));

    let mut wal_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(WAL)
        .expect("Failed to open WAL file");

    let (tx, rx): (Sender<()>, Receiver<()>) = channel();

    let bg_cubic = Arc::clone(&cubicle);
    let bg_dirty = Arc::clone(&dirty);

    thread::spawn(move || {
        loop {
            thread::sleep(std::time::Duration::from_secs(10));

            if bg_dirty.swap(false, Ordering::AcqRel) {
                let snapshot_copy = bg_cubic.read().expect("").clone();
                if let Err(e) = create_snapshot(&snapshot_copy) {
                    eprintln!("Background snapshot failed: {e}")
                } else {
                    let _ = tx.send(());
                }
            }
        }
    });

    loop {
        let mut snap_completed = false;
        while rx.try_recv().is_ok() {
            snap_completed = true;
        }

        if snap_completed && !dirty.load(Ordering::Acquire) {
            if let Ok(new_wal) = OpenOptions::new()
                .create(true)
                .append(true)
                .truncate(true)
                .open(WAL)
            {
                wal_file = new_wal
            }
        }

        println!("Enter a command: ");
        let mut input = String::new();

        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        match input.parse::<Cmd<String>>() {
            Ok(cmd) => match cmd {
                Cmd::Get(key) => {
                    let val = cubicle.read().expect("RwLock poisoned").get(&key).cloned();
                    match val {
                        Some(v) => println!("-> {v}"),
                        None => println!("Key not found"),
                    }
                }
                Cmd::Set(key, value) => {
                    let payload = format!("SET {} {}", key, value);
                    let crc = hash(payload.as_bytes());
                    let log = format!("{:08x} {}\n", crc, payload);

                    if wal_file.write_all(log.as_bytes()).is_ok() && wal_file.flush().is_ok() {
                        cubicle.write().expect("RwLock poisoned").insert(key, value);
                        dirty.store(true, Ordering::Release);
                        println!("-> OK");
                    } else {
                        println!("-> Error: Failed to write to WAL");
                    }
                }
                Cmd::Delete(key) => {
                    let payload = format!("DELETE {}", key);
                    let crc = hash(payload.as_bytes());
                    let log = format!("{:08x} {}\n", crc, payload);

                    if wal_file.write_all(log.as_bytes()).is_ok() && wal_file.flush().is_ok() {
                        let removed = cubicle.write().expect("RwLock poisoned").remove(&key);
                        if removed.is_some() {
                            dirty.store(true, Ordering::Release);
                            println!("-> Deleted");
                        } else {
                            println!("Key not found");
                        }
                    } else {
                        println!("-> Error: Failed to write to WAL");
                    }
                }
                Cmd::See => {
                    let snapshot = cubicle.read().expect("RwLock poisoned").clone();
                    if !snapshot.is_empty() {
                        for (key, value) in snapshot.iter() {
                            println!("{}: {}", key, value);
                        }
                    } else {
                        println!("Cubicle is empty");
                    }
                }
                Cmd::Snapshot => {
                    let snapshot = cubicle.read().expect("RwLock poisoned").clone();
                    if let Err(e) = create_snapshot(&snapshot) {
                        println!("-> Error creating snapshot: {e}");
                    } else {
                        match OpenOptions::new()
                            .create(true)
                            .write(true)
                            .truncate(true)
                            .open(WAL)
                        {
                            Ok(new_wal) => {
                                wal_file = new_wal;
                                dirty.store(false, Ordering::Release);
                                println!("-> Snapshot saved");
                            }
                            Err(e) => println!("-> Failed to truncate WAL: {e}"),
                        }
                    }
                }
            },
            Err(err) => println!("-> Error: {err}"),
        }
    }
}
