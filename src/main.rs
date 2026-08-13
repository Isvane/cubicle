use std::{
    collections::BTreeMap,
    fs::{File, OpenOptions},
    io::{self, BufRead, BufReader, Write},
    str::FromStr,
    sync::{Arc, Mutex},
};

const WAL: &str = "cubicle.wal";

fn main() {
    let mut initial_map = BTreeMap::<i32, String>::new();
    if let Ok(file) = File::open(WAL) {
        let reader = BufReader::new(file);
        for line in reader.lines() {
            if let Ok(line) = line {
                if let Ok(cmd) = line.parse::<Cmd>() {
                    match cmd {
                        Cmd::Set(key, value) => {
                            initial_map.insert(key, value);
                        }
                        Cmd::Put(key, value) => {
                            initial_map.entry(key).and_modify(|m| *m = value);
                        }
                        Cmd::Delete(key) => {
                            initial_map.remove(&key);
                        }
                        _ => {}
                    }
                }
            }
        }
        println!("Restored {} items from WAL", initial_map.len());
    }

    let cubicle = Arc::new(Mutex::new(initial_map));

    let mut wal_file = OpenOptions::new()
        .create(true)
        .append(true)
        .open(WAL)
        .expect("Failed to open WAL file");

    loop {
        println!("Enter a command: ");
        let mut input = String::new();

        io::stdin()
            .read_line(&mut input)
            .expect("Failed to read command");

        match input.parse::<Cmd>() {
            Ok(cmd) => {
                let mut cubic = cubicle.lock().unwrap();
                match cmd {
                    Cmd::Get(key) => match cubic.get(&key) {
                        Some(val) => println!("-> {val}"),
                        None => println!("Keys not found"),
                    },
                    Cmd::Set(key, value) => {
                        let log = format!("SET {} {}\n", key, value);
                        if wal_file.write_all(log.as_bytes()).is_ok() && wal_file.flush().is_ok() {
                            cubic.insert(key, value);
                            println!("-> OK")
                        } else {
                            println!("-> Error: Failed to write to WAL")
                        }
                    }
                    Cmd::Put(key, value) => {
                        if cubic.contains_key(&key) {
                            let log = format!("PUT {} {}\n", key, value);
                            if wal_file.write_all(log.as_bytes()).is_ok()
                                && wal_file.flush().is_ok()
                            {
                                cubic.insert(key, value);
                                println!("-> Updated")
                            } else {
                                println!("-> Error: Failed to write to WAL")
                            }
                        } else {
                            println!("Key not found");
                        }
                    }
                    Cmd::Delete(key) => {
                        let log = format!("DELETE {}\n", key);
                        if wal_file.write_all(log.as_bytes()).is_ok() && wal_file.flush().is_ok() {
                            if cubic.remove(&key).is_some() {
                                println!("-> Deleted")
                            } else {
                                println!("Key not found")
                            }
                        } else {
                            println!("-> Error: Failed to write to WAL")
                        }
                    }
                    Cmd::See => {
                        if !cubic.is_empty() {
                            for (key, value) in cubic.iter() {
                                println!("{}: {}", key, value);
                            }
                        } else {
                            println!("Cubicle is empty")
                        }
                    }
                }
            }
            Err(err) => println!("Error: {err}"),
        }
    }
}

enum Cmd {
    Get(i32),
    Set(i32, String),
    Put(i32, String),
    Delete(i32),
    See,
}

impl FromStr for Cmd {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let mut parts = s.trim().split_whitespace();
        let verb = parts.next().ok_or("Empty input")?.to_uppercase();

        match verb.as_str() {
            "GET" => {
                let key = parts.next().ok_or("GET requires a key")?;
                let key = key.parse::<i32>().map_err(|_| "Key must be integer")?;
                Ok(Cmd::Get(key))
            }
            "SET" => {
                let key = parts.next().ok_or("SET requires a key")?;
                let key = key.parse::<i32>().map_err(|_| "Key must be integer")?;
                let value = parts.collect::<Vec<_>>().join(" ");
                Ok(Cmd::Set(key, value))
            }
            "PUT" => {
                let key = parts.next().ok_or("PUT requires a key")?;
                let key = key.parse::<i32>().map_err(|_| "Key must be integer")?;
                let value = parts.collect::<Vec<_>>().join(" ");
                Ok(Cmd::Put(key, value))
            }
            "DELETE" => {
                let key = parts.next().ok_or("DELETE requires a key")?;
                let key = key.parse::<i32>().map_err(|_| "Key must be integer")?;
                Ok(Cmd::Delete(key))
            }
            "SEE" => Ok(Cmd::See),
            _ => Err(format!("Invalid command {}", verb)),
        }
    }
}
