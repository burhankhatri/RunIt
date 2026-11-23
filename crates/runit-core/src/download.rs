//! Resumable, checksummed downloads.
use crate::catalog::hex;
use crate::http;
use sha2::{Digest, Sha256};
use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::mpsc::{sync_channel, Receiver, RecvTimeoutError};
use std::time::Duration;

/// Where a remote file goes. Remote names are not trusted.
pub fn safe_path(root: &Path, repo: &str, path: &str) -> Result<PathBuf, String> {
    let parts: Vec<&str> = repo.split('/').chain(path.split('/')).collect();
    let fine = |p: &str| !p.is_empty() && p != "." && p != ".." && !p.ends_with('.') && !p.contains(|c: char| c.is_control() || "\\:*?\"<>|".contains(c));
    if repo.split('/').count() != 2 || !parts.iter().all(|p| fine(p)) {
        return Err(format!("refusing an unsafe file name: {repo}/{path}"));
    }
    Ok(parts.iter().fold(root.to_path_buf(), |p, s| p.join(s)))
}

pub fn part_path(dest: &Path) -> PathBuf {
    let mut n = dest.file_name().unwrap_or_default().to_os_string();
    n.push(".part");
    dest.with_file_name(n)
}

/// Read on another thread, so a silent connection can be noticed and left behind.
fn pump(mut reader: Box<dyn Read + Send>) -> Receiver<Result<Vec<u8>, String>> {
    let (tx, rx) = sync_channel(4);
    std::thread::spawn(move || loop {
        let mut v = vec![0; 256 << 10];
        let item = match reader.read(&mut v) {
            Ok(0) => return,
            Ok(n) => {
                v.truncate(n);
                Ok(v)
            }
            Err(e) => Err(format!("the connection dropped: {e}")),
        };
        let last = item.is_err();
        if tx.send(item).is_err() || last {
            return;
        }
    });
    rx
}

/// Fetch `url` to `dest`, continuing a `.part` file if there is one. Reopens the link when the
/// connection drops, and only moves the file into place once its SHA-256 matches.
pub fn fetch(url: &str, dest: &Path, size: u64, sha256: &str, progress: &mut dyn FnMut(u64), stop: &AtomicBool) -> Result<(), String> {
    let part = part_path(dest);
    let io = |e: std::io::Error| match e.kind() {
        std::io::ErrorKind::StorageFull => "the disk is full".to_string(),
        _ => format!("could not write the file: {e}"),
    };
    std::fs::create_dir_all(dest.parent().ok_or("no folder")?).map_err(io)?;

    let mut hash = Sha256::new();
    let mut done = 0u64;
    let mut buf = vec![0u8; 256 << 10];
    if part.metadata().is_ok_and(|m| m.len() > size) {
        std::fs::remove_file(&part).map_err(io)?;
    }
    if let Ok(mut f) = File::open(&part) {
        loop {
            let n = f.read(&mut buf).map_err(io)?;
            if n == 0 {
                break;
            }
            hash.update(&buf[..n]);
            done += n as u64;
            if stop.load(Ordering::Relaxed) {
                return Err("stopped".into());
            }
        }
    }
    let mut out = OpenOptions::new().create(true).append(true).open(&part).map_err(io)?;

    let mut failures = 0;
    while done < size {
        let before = done;
        let err = match http::open(url, done, None) {
            Ok(body) => {
                if done > 0 && body.status != 206 {
                    // Range ignored: start over rather than append the beginning again.
                    // Windows does not let a file opened for appending be cut short.
                    drop(File::create(&part).map_err(io)?);
                    hash = Sha256::new();
                    done = 0;
                }
                let chunks = pump(body.reader);
                let mut quiet = 0;
                loop {
                    if stop.load(Ordering::Relaxed) {
                        return Err("stopped".into());
                    }
                    match chunks.recv_timeout(Duration::from_millis(250)) {
                        Ok(Ok(chunk)) => {
                            quiet = 0;
                            let n = chunk.len().min((size - done) as usize);
                            out.write_all(&chunk[..n]).map_err(io)?;
                            hash.update(&chunk[..n]);
                            done += n as u64;
                            progress(done);
                            if done == size {
                                break String::new();
                            }
                        }
                        Ok(Err(e)) => break e,
                        Err(RecvTimeoutError::Disconnected) => break "the connection closed early".to_string(),
                        Err(RecvTimeoutError::Timeout) if quiet >= 120 => break "the connection stalled".to_string(),
                        Err(RecvTimeoutError::Timeout) => quiet += 1,
                    }
                }
            }
            Err(e) => e,
        };
        if done == size {
            break;
        }
        failures = if done > before { 0 } else { failures + 1 };
        if failures >= 5 {
            return Err(err);
        }
        for _ in 0..(1 << failures) * 5 {
            if stop.load(Ordering::Relaxed) {
                return Err("stopped".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }
    out.sync_all().map_err(io)?;
    drop(out);
    if hex(&hash.finalize()) != sha256.to_lowercase() {
        std::fs::remove_file(&part).map_err(io)?;
        return Err("the file did not match its published checksum, so it was deleted".into());
    }
    progress(size);
    std::fs::rename(&part, dest).map_err(io)
}
