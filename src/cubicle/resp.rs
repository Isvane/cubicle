use bytes::{Buf, BytesMut};
use std::io::{self, Cursor};

#[derive(Debug, Clone, PartialEq)]
pub enum Frame {
    SimpleString(String),
    Error(String),
    Integer(i64),
    BulkString(Option<Vec<u8>>),
    Array(Option<Vec<Frame>>),
    Null,
}

impl Frame {
    pub fn to_bytes(&self) -> Vec<u8> {
        let mut buf = Vec::new();
        self.serialize(&mut buf);
        buf
    }

    pub fn serialize(&self, buf: &mut Vec<u8>) {
        match self {
            Frame::SimpleString(s) => {
                buf.push(b'+');
                buf.extend_from_slice(s.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            Frame::Error(e) => {
                buf.push(b'-');
                buf.extend_from_slice(e.as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            Frame::Integer(i) => {
                buf.push(b':');
                buf.extend_from_slice(i.to_string().as_bytes());
                buf.extend_from_slice(b"\r\n");
            }
            Frame::BulkString(b) => {
                if let Some(bytes) = b {
                    buf.push(b'$');
                    buf.extend_from_slice(bytes.len().to_string().as_bytes());
                    buf.extend_from_slice(b"\r\n");
                    buf.extend_from_slice(bytes);
                    buf.extend_from_slice(b"\r\n");
                } else {
                    buf.extend_from_slice(b"$-1\r\n");
                }
            }
            Frame::Array(a) => {
                if let Some(frames) = a {
                    buf.push(b'*');
                    buf.extend_from_slice(frames.len().to_string().as_bytes());
                    buf.extend_from_slice(b"\r\n");

                    for element in frames {
                        element.serialize(buf);
                    }
                } else {
                    buf.extend_from_slice(b"*-1\r\n");
                }
            }
            Frame::Null => {
                buf.push(b'_');
                buf.extend_from_slice(b"\r\n");
            }
        }
    }

    pub fn parser(src: &mut BytesMut) -> Result<Option<Self>, io::Error> {
        let mut cursor = Cursor::new(&src[..]);

        match Self::parse_cursor(&mut cursor)? {
            Some(frame) => {
                let consumed = cursor.position() as usize;
                src.advance(consumed);
                Ok(Some(frame))
            }
            None => Ok(None),
        }
    }

    fn parse_cursor(cursor: &mut Cursor<&[u8]>) -> Result<Option<Self>, io::Error> {
        if !cursor.has_remaining() {
            return Ok(None);
        }

        let pos = cursor.position() as usize;
        let tag = cursor.get_ref()[pos];

        match tag {
            b'+' => Self::parse_simple_string(cursor),
            b'-' => Self::parse_error(cursor),
            b':' => Self::parse_integer(cursor),
            b'$' => Self::parse_bulk_string(cursor),
            b'*' => Self::parse_array(cursor),
            b'_' => {
                if cursor.remaining() < 3 {
                    return Ok(None);
                }
                if &cursor.get_ref()[pos..pos + 3] == b"_\r\n" {
                    cursor.advance(3);
                    Ok(Some(Frame::Null))
                } else {
                    Err(io::Error::new(
                        io::ErrorKind::InvalidData,
                        "Invalid Null frame",
                    ))
                }
            }
            _ => Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid RESP type",
            )),
        }
    }

    fn parse_simple_string(cursor: &mut Cursor<&[u8]>) -> Result<Option<Self>, io::Error> {
        match read_line(cursor) {
            Some(bytes) => {
                let txt = std::str::from_utf8(bytes)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?
                    .to_string();
                Ok(Some(Self::SimpleString(txt)))
            }
            None => Ok(None),
        }
    }

    fn parse_error(cursor: &mut Cursor<&[u8]>) -> Result<Option<Self>, io::Error> {
        match read_line(cursor) {
            Some(bytes) => {
                let err_msg = std::str::from_utf8(bytes)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?
                    .to_string();
                Ok(Some(Self::Error(err_msg)))
            }
            None => Ok(None),
        }
    }

    fn parse_integer(cursor: &mut Cursor<&[u8]>) -> Result<Option<Self>, io::Error> {
        match read_line(cursor) {
            Some(bytes) => {
                let num_str = std::str::from_utf8(bytes)
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8"))?;
                let number = num_str
                    .parse::<i64>()
                    .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid integer"))?;
                Ok(Some(Self::Integer(number)))
            }
            None => Ok(None),
        }
    }

    fn parse_bulk_string(cursor: &mut Cursor<&[u8]>) -> Result<Option<Self>, io::Error> {
        if !cursor.has_remaining() {
            return Ok(None);
        }

        let pos = cursor.position() as usize;
        if cursor.get_ref()[pos] != b'$' {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Invalid Bulk string",
            ));
        }

        let length_bytes = match read_line(cursor) {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let length_str = std::str::from_utf8(length_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 length"))?;
        let length: i64 = length_str
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid integer length"))?;

        if length == -1 {
            return Ok(Some(Self::BulkString(None)));
        }
        if length < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Length can't be < 0",
            ));
        }

        let len = length as usize;
        let start = cursor.position() as usize;
        let available = cursor.get_ref().len() - start;

        if available < len + 2 {
            return Ok(None);
        }

        let payload = cursor.get_ref()[start..start + len].to_vec();

        if &cursor.get_ref()[start + len..start + len + 2] != b"\r\n" {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Bulk string payload does not end with CRLF",
            ));
        }

        cursor.set_position((start + len + 2) as u64);

        Ok(Some(Self::BulkString(Some(payload))))
    }

    fn parse_array(cursor: &mut Cursor<&[u8]>) -> Result<Option<Self>, io::Error> {
        if !cursor.has_remaining() {
            return Ok(None);
        }

        let pos = cursor.position() as usize;
        if cursor.get_ref()[pos] != b'*' {
            return Err(io::Error::new(io::ErrorKind::InvalidData, "Invalid Array"));
        }

        let length_bytes = match read_line(cursor) {
            Some(bytes) => bytes,
            None => return Ok(None),
        };

        let length_str = std::str::from_utf8(length_bytes)
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid UTF-8 length"))?;
        let length: i64 = length_str
            .parse()
            .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "Invalid integer length"))?;

        if length == -1 {
            return Ok(Some(Self::Array(None)));
        }
        if length < 0 {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Length can't be < 0",
            ));
        }

        let mut elements = Vec::with_capacity(length as usize);

        for _ in 0..length {
            match Self::parse_cursor(cursor)? {
                Some(frame) => elements.push(frame),
                None => return Ok(None),
            }
        }

        Ok(Some(Self::Array(Some(elements))))
    }
}

fn read_line<'a>(cursor: &mut Cursor<&'a [u8]>) -> Option<&'a [u8]> {
    let start = cursor.position() as usize;
    let slice = &cursor.get_ref()[start..];

    let line_end = slice.windows(2).position(|w| w == b"\r\n");

    if let Some(crlf) = line_end {
        let line = &slice[1..crlf];
        cursor.set_position((start + crlf + 2) as u64);
        Some(line)
    } else {
        None
    }
}
