use std::{
    collections::BTreeMap,
    fs::File,
    io::{self, BufRead, BufReader, Write},
};

use crate::cubicle::cmd::{Cmd, Value};

pub(crate) const WAL: &str = "cubicle.wal";
pub(crate) const SNAPSHOT: &str = "cubicle.snap";

pub fn restore_state() -> BTreeMap<String, Value> {
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

pub fn create_snapshot(map: &BTreeMap<String, Value>) -> io::Result<()> {
    let temp_path = "cubicle.snap.tmp";

    let mut file = File::create(temp_path)?;
    for (key, val) in map {
        writeln!(file, "SET {} {}", key, val)?;
    }
    file.flush()?;

    std::fs::rename(temp_path, SNAPSHOT)?;
    Ok(())
}
