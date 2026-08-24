use std::{
    fs::OpenOptions,
    io::{self, Write},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
        mpsc::{Receiver, Sender, channel},
    },
    thread,
};

use im::OrdMap;

mod cubicle;

use cubicle::cmd::{Cmd, Value};
use cubicle::persistence::{WAL, create_snapshot, restore_state};

fn main() {
    let initial_map: OrdMap<String, Value> = restore_state();
    println!("Restored {} items from disk", initial_map.len());

    let cubicle: Arc<Mutex<OrdMap<String, Value>>> = Arc::new(Mutex::new(initial_map));
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
                let snapshot_copy = bg_cubic.lock().unwrap().clone();
                if let Err(e) = create_snapshot(&snapshot_copy) {
                    eprintln!("Background snapshot failed: {e}")
                } else {
                    let _ = tx.send(());
                }
            }
        }
    });

    loop {
        if rx.try_recv().is_ok()
            && let Ok(new_wal) = OpenOptions::new()
                .create(true)
                .write(true)
                .truncate(true)
                .open(WAL)
        {
            wal_file = new_wal;
        }

        println!("Enter a command: ");
        let mut input = String::new();

        if io::stdin().read_line(&mut input).is_err() {
            break;
        }

        match input.parse::<Cmd<String>>() {
            Ok(cmd) => {
                let mut cubic = cubicle.lock().unwrap();
                match cmd {
                    Cmd::Get(key) => match cubic.get(&key) {
                        Some(val) => println!("-> {val}"),
                        None => println!("Key not found"),
                    },
                    Cmd::Set(key, value) => {
                        let log = format!("SET {} {}\n", key, value);
                        if wal_file.write_all(log.as_bytes()).is_ok() && wal_file.flush().is_ok() {
                            cubic.insert(key, value);
                            dirty.store(true, Ordering::Release);
                            println!("-> OK");
                        } else {
                            println!("-> Error: Failed to write to WAL");
                        }
                    }
                    Cmd::Put(key, value) => {
                        if cubic.contains_key(&key) {
                            let log = format!("PUT {} {}\n", key, value);
                            if wal_file.write_all(log.as_bytes()).is_ok()
                                && wal_file.flush().is_ok()
                            {
                                cubic.insert(key, value);
                                dirty.store(true, Ordering::Release);
                                println!("-> Updated");
                            } else {
                                println!("-> Error: Failed to write to WAL");
                            }
                        } else {
                            println!("Key not found");
                        }
                    }
                    Cmd::Delete(key) => {
                        let log = format!("DELETE {}\n", key);
                        if wal_file.write_all(log.as_bytes()).is_ok() && wal_file.flush().is_ok() {
                            if cubic.remove(&key).is_some() {
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
                        if !cubic.is_empty() {
                            for (key, value) in cubic.iter() {
                                println!("{}: {}", key, value);
                            }
                        } else {
                            println!("Cubicle is empty");
                        }
                    }
                    Cmd::Snapshot => {
                        if let Err(e) = create_snapshot(&cubic) {
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
                                    println!("-> Snapshot saved");
                                }
                                Err(e) => println!("-> Failed to truncate WAL: {e}"),
                            }
                        }
                    }
                }
            }
            Err(err) => println!("-> Error: {err}"),
        }
    }
}
