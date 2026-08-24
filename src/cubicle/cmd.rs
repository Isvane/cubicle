use std::{fmt, str::FromStr};

pub(crate) enum Cmd<T> {
    Get(T),
    Set(T, Value),
    Delete(T),
    See,
    Snapshot,
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<Value>),
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
                let value_str = parts.collect::<Vec<_>>().join(" ");
                let value = value_str.parse::<Value>()?;
                Ok(Cmd::Set(key, value))
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

impl FromStr for Value {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim();

        if trimmed.starts_with('[') && trimmed.ends_with(']') {
            let inner = &trimmed[1..trimmed.len() - 1];
            if inner.trim().is_empty() {
                return Ok(Value::List(Vec::new()));
            }

            let elements = inner
                .split(',')
                .map(|item| item.trim().parse::<Value>())
                .collect::<Result<Vec<Value>, String>>()?;

            return Ok(Value::List(elements));
        }

        if let Ok(val) = trimmed.parse::<i64>() {
            return Ok(Value::Integer(val));
        }
        if let Ok(val) = trimmed.parse::<f64>() {
            return Ok(Value::Float(val));
        }
        if let Ok(val) = trimmed.parse::<bool>() {
            return Ok(Value::Boolean(val));
        }

        if trimmed.starts_with('"') && trimmed.ends_with('"') && trimmed.len() >= 2 {
            Ok(Value::String(trimmed[1..trimmed.len() - 1].to_string()))
        } else {
            Ok(Value::String(s.to_string()))
        }
    }
}

impl std::fmt::Display for Value {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Value::String(s) => write!(f, "\"{s}\""),
            Value::Integer(i) => write!(f, "{i}"),
            Value::Boolean(b) => write!(f, "{b}"),
            Value::Float(fl) => write!(f, "{fl}"),
            Value::List(l) => {
                let items: Vec<String> = l.iter().map(|v| v.to_string()).collect();
                write!(f, "[{}]", items.join(", "))
            }
        }
    }
}
