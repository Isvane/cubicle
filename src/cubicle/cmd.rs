use std::{fmt, str::FromStr};

pub(crate) enum Cmd<T> {
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
