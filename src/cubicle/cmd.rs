use crate::cubicle::resp::Frame;

use std::str::FromStr;

pub(crate) enum Cmd<T> {
    Get(T),
    Set(T, Value),
    Delete(T),
    See,
    Snapshot,
}

impl TryFrom<Frame> for Cmd<String> {
    type Error = String;

    fn try_from(frame: Frame) -> Result<Self, Self::Error> {
        let frames = match frame {
            Frame::Array(Some(arr)) => arr,
            _ => return Err("ERR Protocol error: expected array".to_string()),
        };

        if frames.is_empty() {
            return Err("ERR Protocol error: empty command".to_string());
        }

        let get_string = |f: &Frame| -> Result<String, String> {
            match f {
                Frame::BulkString(Some(bytes)) => String::from_utf8(bytes.clone())
                    .map_err(|_| "ERR Invalid UTF-8 in string".to_string()),
                Frame::SimpleString(s) => Ok(s.clone()),
                _ => Err("ERR Protocol error: expected string".to_string()),
            }
        };

        let cmd_name = get_string(&frames[0])?.to_uppercase();

        match cmd_name.as_str() {
            "GET" => {
                if frames.len() != 2 {
                    return Err("ERR wrong number of arguments for 'get' command".to_string());
                }
                Ok(Cmd::Get(get_string(&frames[1])?))
            }
            "SET" => {
                if frames.len() != 3 {
                    return Err("ERR wrong number of arguments for 'set' command".to_string());
                }
                let val_str = get_string(&frames[2])?;
                let value = val_str.parse::<Value>()?;
                Ok(Cmd::Set(get_string(&frames[1])?, value))
            }
            "DEL" | "DELETE" => {
                if frames.len() != 2 {
                    return Err("ERR wrong number of arguments for 'del' command".to_string());
                }
                Ok(Cmd::Delete(get_string(&frames[1])?))
            }
            "SEE" | "KEYS" => Ok(Cmd::See),
            "SNAPSHOT" | "SAVE" | "BGSAVE" => Ok(Cmd::Snapshot),
            _ => Err(format!("ERR unknown command '{}'", cmd_name)),
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    String(String),
    Integer(i64),
    Float(f64),
    Boolean(bool),
    List(Vec<Value>),
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
            Value::String(s) => write!(f, "{s}"),
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
