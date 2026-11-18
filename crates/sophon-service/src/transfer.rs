use crate::host::envelope;
use cap_std::{ambient_authority, fs::Dir};
use futures_util::SinkExt;
use serde_json::{Value, json};
use std::{
    collections::HashMap,
    io::{Read, Write},
    path::Path,
};
use tempfile::NamedTempFile;
use tokio::net::TcpStream;
use tokio_tungstenite::{WebSocketStream, tungstenite::Message};

const LIMIT: u64 = 64 * 1024 * 1024;
const CHUNK: usize = 128 * 1024;
type Error = Box<dyn std::error::Error>;

pub struct Upload {
    file: NamedTempFile,
    metadata: Value,
    begun: bool,
    written: u64,
}

impl Upload {
    pub fn begin(
        home: &Path,
        request: &Value,
        uploads: &mut HashMap<String, Self>,
    ) -> Result<(), Error> {
        let id = request["requestId"]
            .as_str()
            .filter(|id| !id.is_empty() && id.len() <= 255)
            .ok_or("Invalid request ID")?;
        let name = request["fileName"]
            .as_str()
            .filter(|name| !name.is_empty() && name.len() <= 255)
            .ok_or("Invalid file name")?;
        if Path::new(name).file_name().and_then(|part| part.to_str()) != Some(name) {
            return Err("Invalid file name".into());
        }
        if request["size"].as_u64().is_none_or(|size| size > LIMIT)
            || uploads.len() >= 4
            || uploads.contains_key(id)
        {
            return Err("Upload limit exceeded".into());
        }
        let root = home.join("uploads");
        std::fs::create_dir_all(&root)?;
        let file = NamedTempFile::new_in(root)?;
        uploads.insert(
            id.to_owned(),
            Self {
                file,
                metadata: request.clone(),
                begun: false,
                written: 0,
            },
        );
        Ok(())
    }

    pub fn receive(
        uploads: &mut HashMap<String, Self>,
        bytes: &[u8],
    ) -> Result<Option<Value>, Error> {
        let (&opcode, rest) = bytes.split_first().ok_or("Missing opcode")?;
        let (&length, rest) = rest.split_first().ok_or("Missing request ID")?;
        let (id, body) = rest
            .split_at_checked(length as usize)
            .ok_or("Invalid request ID")?;
        let id = std::str::from_utf8(id)?;
        let Some(upload) = uploads.get_mut(id) else {
            return Ok(None);
        };
        let size = upload.metadata["size"].as_u64().ok_or("Invalid size")?;
        match opcode {
            0x10 if !upload.begun => upload.begun = true,
            0x11 if upload.begun
                && body.len() <= CHUNK
                && upload.written + body.len() as u64 <= size =>
            {
                upload.file.write_all(body)?;
                upload.written += body.len() as u64;
            }
            0x12 if upload.begun && upload.written == size => {
                let upload = uploads.remove(id).unwrap();
                let (_, path) = upload.file.keep()?;
                return Ok(Some(envelope(
                    "file.upload.response",
                    json!({"requestId":id,"file":{
                    "type":"file","path":path,"fileName":upload.metadata["fileName"],"mimeType":upload.metadata["mimeType"],"size":size,
                    "modifiedAt":upload.metadata["modifiedAt"]}}),
                )));
            }
            _ => {
                uploads.remove(id);
                return Ok(Some(envelope(
                    "file.upload.response",
                    json!({"requestId":id,"error":"Invalid file transfer"}),
                )));
            }
        }
        Ok(None)
    }
}

/// Removes what was uploaded more than a week ago: a file sent for a
/// message is read during that message's turn, and nothing reads it
/// after.
pub fn prune(home: &Path) {
    let Ok(uploads) = std::fs::read_dir(home.join("uploads")) else {
        return;
    };
    let week = std::time::Duration::from_secs(7 * 24 * 60 * 60);
    for upload in uploads.flatten() {
        let old = upload
            .metadata()
            .and_then(|metadata| metadata.modified())
            .is_ok_and(|modified| modified.elapsed().is_ok_and(|age| age > week));
        if old {
            let _ = std::fs::remove_file(upload.path());
        }
    }
}

pub async fn download(
    socket: &mut WebSocketStream<TcpStream>,
    cwd: &str,
    request: &Value,
) -> Result<(), Error> {
    let path = request["path"].as_str().ok_or("Missing path")?;
    let id = request["requestId"]
        .as_str()
        .filter(|id| !id.is_empty() && id.len() <= 255)
        .ok_or("Invalid request ID")?;
    let dir = Dir::open_ambient_dir(cwd, ambient_authority())?;
    // Asked before opening: opening a pipe waits for its other end.
    if !dir.metadata(path)?.is_file() {
        return Err("Not a regular file".into());
    }
    let mut file = dir.open(path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > LIMIT {
        return Err("File exceeds download limit or is not a regular file".into());
    }
    let header = |opcode| {
        let mut bytes = vec![opcode, id.len() as u8];
        bytes.extend_from_slice(id.as_bytes());
        bytes
    };
    let meta = serde_json::to_vec(
        &json!({"size":metadata.len(),"encoding":"binary","fileName":Path::new(path).file_name().and_then(|part|part.to_str())}),
    )?;
    let mut start = header(0x10);
    start.extend_from_slice(&u16::try_from(meta.len())?.to_be_bytes());
    start.extend_from_slice(&meta);
    socket.send(Message::binary(start)).await?;
    let mut bytes = vec![0; CHUNK];
    let mut remaining = metadata.len();
    while remaining > 0 {
        let limit = remaining.min(CHUNK as u64) as usize;
        let count = file.read(&mut bytes[..limit])?;
        if count == 0 {
            return Err("File changed during download".into());
        }
        let mut frame = header(0x11);
        frame.extend_from_slice(&bytes[..count]);
        socket.send(Message::binary(frame)).await?;
        remaining -= count as u64;
    }
    socket.send(Message::binary(header(0x12))).await?;
    Ok(())
}
