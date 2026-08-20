use std::{
    collections::BTreeMap,
    fmt,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    str::FromStr,
    sync::{Arc, Mutex, mpsc::channel},
    thread,
};

const WAL: &str = "cubicle.wal";
const SNAPSHOT: &str = "cubicle.snap";

fn main() {
    let initial_map = restore_state();
    println!("Restored {} items from disk", initial_map.len());

    let cubicle = Arc::new(Mutex::new(initial_map));

    let mut wal_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(WAL)
        .expect("Failed to open WAL file");

    let (tx, rx) = channel::<BTreeMap<String, String>>();

    thread::spawn(move || {
        while let Ok(snapshot_data) = rx.recv() {
            if let Err(e) = create_snapshot(&snapshot_data) {
                eprintln!("Background snapshot failed: {e}");
            }
        }
    });

    let bg_cubic = Arc::clone(&cubicle);
    let bg_tx = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(std::time::Duration::from_secs(15));
            let snapshot_copy = bg_cubic.lock().unwrap().clone();
            let _ = bg_tx.send(snapshot_copy);
        }
    });

    loop {
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

fn restore_state() -> BTreeMap<String, String> {
    let mut map = BTreeMap::new();

    if let Ok(file) = File::open(SNAPSHOT) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(Cmd::Set(key, value)) = line.parse::<Cmd<String>>() {
                map.insert(key, value);
            }
        }
    }

    if let Ok(file) = File::open(WAL) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            if let Ok(cmd) = line.parse::<Cmd<String>>() {
                match cmd {
                    Cmd::Set(key, value) => {
                        map.insert(key, value);
                    }
                    Cmd::Put(key, value) => {
                        map.entry(key).and_modify(|m| *m = value);
                    }
                    Cmd::Delete(key) => {
                        map.remove(&key);
                    }
                    _ => {}
                }
            }
        }
    }

    map
}

fn create_snapshot(map: &BTreeMap<String, String>) -> io::Result<()> {
    let temp_path = "cubicle.snap.tmp";

    let mut file = File::create(temp_path)?;
    for (key, val) in map {
        writeln!(file, "SET {} {}", key, val)?;
    }
    file.flush()?;

    std::fs::rename(temp_path, SNAPSHOT)?;
    Ok(())
}

enum Cmd<T> {
    Get(T),
    Set(T, String),
    Put(T, String),
    Delete(T),
    See,
    Snapshot,
}

impl<T> FromStr for Cmd<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.split_whitespace();
        let verb = parts.next().ok_or("Empty input")?.to_uppercase();

        match verb.as_str() {
            "GET" => {
                let key = parts.next().ok_or("GET requires a key")?;
                let key = key.parse::<T>().map_err(|e| format!("{e}"))?;
                Ok(Cmd::Get(key))
            }
            "SET" => {
                let key = parts.next().ok_or("SET requires a key")?;
                let key = key.parse::<T>().map_err(|e| format!("{e}"))?;
                let value = parts.collect::<Vec<_>>().join(" ");
                Ok(Cmd::Set(key, value))
            }
            "PUT" => {
                let key = parts.next().ok_or("PUT requires a key")?;
                let key = key.parse::<T>().map_err(|e| format!("{e}"))?;
                let value = parts.collect::<Vec<_>>().join(" ");
                Ok(Cmd::Put(key, value))
            }
            "DELETE" => {
                let key = parts.next().ok_or("DELETE requires a key")?;
                let key = key.parse::<T>().map_err(|e| format!("{e}"))?;
                Ok(Cmd::Delete(key))
            }
            "SEE" => Ok(Cmd::See),
            "SNAPSHOT" => Ok(Cmd::Snapshot),
            _ => Err(format!("Invalid command {}", verb)),
        }
    }
}
