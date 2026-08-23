use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, BufRead, BufReader, Write},
};

use crc32fast::hash;

use crate::cubicle::cmd::{Cmd, Value};

pub(crate) const WAL: &str = "cubicle.wal";
pub(crate) const SNAPSHOT: &str = "cubicle.snap";

pub fn restore_state() -> BTreeMap<String, Value> {
    let mut map = BTreeMap::new();

    if let Ok(file) = File::open(SNAPSHOT) {
        let reader = BufReader::new(file);
        for line in reader.lines().map_while(Result::ok) {
            if let Some((crc_hex, payload)) =
                line.strip_prefix("SET ").and_then(|r| r.split_once(' '))
            {
                if let Ok(expected_crc) = u32::from_str_radix(crc_hex, 16) {
                    let actual_crc = hash(payload.as_bytes());
                    if actual_crc == expected_crc {
                        if let Some((key, value_str)) = payload.split_once(' ') {
                            let parsed_val = value_str
                                .parse::<Value>()
                                .unwrap_or_else(|_| Value::String(value_str.to_string()));
                            map.insert(key.to_string(), parsed_val);
                        }
                    }
                }
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

pub fn create_snapshot(map: &BTreeMap<String, Value>) -> io::Result<()> {
    let temp_path = "cubicle.snap.tmp";

    let mut file = File::create(temp_path)?;
    for (key, val) in map {
        let payload = format!("{} {}", key, val);

        let crc = hash(payload.as_bytes());
        let crc_hex = format!("{:08x}", crc);

        writeln!(file, "SET {} {}", crc_hex, payload)?;
    }
    file.flush()?;

    std::fs::rename(temp_path, SNAPSHOT)?;
    Ok(())
}
