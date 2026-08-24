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

fn extract_key(s: &str) -> Result<(&str, &str), String> {
    let s = s.trim_start();
    if s.is_empty() {
        return Err("Missing required key argument".to_string());
    }

    if s.starts_with('"') {
        let closing = s[1..]
            .find('"')
            .map(|i| i + 1)
            .ok_or("Unclosed quote in key")?;
        Ok((&s[1..closing], &s[closing + 1..]))
    } else {
        let end = s.find(char::is_whitespace).unwrap_or(s.len());
        Ok((&s[..end], &s[end..]))
    }
}

impl<T> FromStr for Cmd<T>
where
    T: FromStr,
    T::Err: fmt::Display,
{
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        let trimmed = s.trim_start();
        if trimmed.is_empty() {
            return Err("Empty input".to_string());
        }

        let (verb, remainder) = match trimmed.find(char::is_whitespace) {
            Some(idx) => (&trimmed[..idx], &trimmed[idx..]),
            None => (trimmed, ""),
        };

        let verb = verb.to_uppercase();

        match verb.as_str() {
            "GET" => {
                let (key_str, _) = extract_key(remainder)?;
                let key = key_str.parse::<T>().map_err(|e| format!("{e}"))?;
                Ok(Cmd::Get(key))
            }
            "SET" => {
                let (key_str, value_str) = extract_key(remainder)?;
                let key = key_str.parse::<T>().map_err(|e| format!("{e}"))?;
                let value_str = value_str.trim();
                if value_str.is_empty() {
                    return Err("SET requires a value".to_string());
                }
                let value = value_str.parse::<Value>()?;
                Ok(Cmd::Set(key, value))
            }
            "DELETE" => {
                let (key_str, _) = extract_key(remainder)?;
                let key = key_str.parse::<T>().map_err(|e| format!("{e}"))?;
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
