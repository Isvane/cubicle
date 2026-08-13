use std::{
    collections::BTreeMap,
    io,
    str::FromStr,
    sync::{Arc, Mutex},
};

fn main() {
    let cubicle = Arc::new(Mutex::new(BTreeMap::<i32, String>::new()));

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
                        cubic.insert(key, value);
                        println!("-> OK")
                    }
                    Cmd::Delete(key) => {
                        if cubic.remove(&key).is_some() {
                            println!("-> Deleted")
                        } else {
                            println!("Key not found")
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
