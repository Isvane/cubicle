use std::io;

use bytes::{Buf, BytesMut};
use crc32fast::hash;
use im::OrdMap;

use tokio::{
    fs::{self, File},
    io::{AsyncReadExt, AsyncWriteExt},
};

use crate::cubicle::{
    cmd::{Cmd, Value},
    resp::Frame,
};

pub(crate) const WAL: &str = "cubicle.wal";

pub(crate) const SNAPSHOT: &str = "cubicle.snap";

pub async fn restore_state() -> OrdMap<String, Value> {
    let mut map = OrdMap::new();

    for filename in [SNAPSHOT, WAL] {
        if let Ok(mut file) = File::open(filename).await {
            let mut buffer = BytesMut::new();

            'file_read: loop {
                match file.read_buf(&mut buffer).await {
                    Ok(0) => break 'file_read,
                    Ok(_) => {}
                    Err(_) => break 'file_read,
                }

                loop {
                    if buffer.len() < 9 {
                        break;
                    }

                    let crc_str = match std::str::from_utf8(&buffer[..8]) {
                        Ok(s) => s,

                        Err(_) => break 'file_read,
                    };
                    let expected_crc = match u32::from_str_radix(crc_str, 16) {
                        Ok(crc) => crc,

                        Err(_) => break 'file_read,
                    };

                    let mut frame_buf = BytesMut::from(&buffer[9..]);

                    match Frame::parser(&mut frame_buf) {
                        Ok(Some(frame)) => {
                            let frame_bytes_len = (buffer.len() - 9) - frame_buf.len();

                            let payload_bytes = &buffer[9..9 + frame_bytes_len];
                            if hash(payload_bytes) != expected_crc {
                                break 'file_read;
                            }

                            if let Ok(cmd) = Cmd::try_from(frame) {
                                match cmd {
                                    Cmd::Set(key, value) => {
                                        map.insert(key, value);
                                    }

                                    Cmd::Delete(key) => {
                                        map.remove(&key);
                                    }

                                    _ => {}
                                }
                            }

                            buffer.advance(9 + frame_bytes_len);
                        }

                        Ok(None) => {
                            break;
                        }
                        Err(_) => {
                            break 'file_read;
                        }
                    }
                }
            }
        }
    }

    map
}

pub async fn create_snapshot(map: &OrdMap<String, Value>) -> io::Result<()> {
    let temp_path = "cubicle.snap.tmp";

    let mut file = File::create(temp_path).await?;

    for (key, val) in map {
        let payload = vec![
            Frame::BulkString(Some("SET".as_bytes().to_vec())),
            Frame::BulkString(Some(key.as_bytes().to_vec())),
            Frame::BulkString(Some(val.to_string().into_bytes())),
        ];

        let arr = Frame::Array(Some(payload));

        let resp_bytes = arr.to_bytes();

        let crc = hash(&resp_bytes);
        let crc_header = format!("{:08x} ", crc);

        file.write_all(crc_header.as_bytes()).await?;
        file.write_all(&resp_bytes).await?;
    }

    file.flush().await?;

    fs::rename(temp_path, SNAPSHOT).await?;

    Ok(())
}
