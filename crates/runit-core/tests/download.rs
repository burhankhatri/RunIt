use runit_core::download::*;
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::{Path, PathBuf};
use std::sync::{atomic::{AtomicBool, AtomicUsize, Ordering}, Arc, Mutex};

/// A file server behind a redirect. `cut` closes the first connections early; `plain` ignores Range.
struct Server { url: String, ranges: Arc<Mutex<Vec<String>>>, opens: Arc<AtomicUsize> }
fn serve(data: Vec<u8>, cut: usize, plain: bool, busy_first: bool) -> Server {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let base = format!("http://{}", l.local_addr().unwrap());
    let (ranges, opens) = (Arc::new(Mutex::new(vec![])), Arc::new(AtomicUsize::new(0)));
    let (r2, o2, b2) = (ranges.clone(), opens.clone(), base.clone());
    std::thread::spawn(move || {
        let (mut cuts, mut busy) = (cut, busy_first);
        for s in l.incoming().flatten() {
            let mut s = s; let mut lines = BufReader::new(s.try_clone().unwrap()).lines();
            let first = lines.next().and_then(Result::ok).unwrap_or_default();
            let mut range = String::new();
            for h in lines.map_while(Result::ok) { if h.is_empty() { break; } if let Some(v) = h.to_lowercase().strip_prefix("range: ") { range = v.into(); } }
            if first.contains("/go") {
                o2.fetch_add(1, Ordering::SeqCst);
                if std::mem::take(&mut busy) { let _ = write!(s, "HTTP/1.1 503 Busy\r\nRetry-After: 0\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"); continue; }
                let _ = write!(s, "HTTP/1.1 302 Found\r\nLocation: {b2}/data\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"); continue;
            }
            r2.lock().unwrap().push(range.clone());
            let from: usize = if plain { 0 } else { range.strip_prefix("bytes=").and_then(|r| r.split('-').next()).and_then(|n| n.parse().ok()).unwrap_or(0) };
            let body = &data[from.min(data.len())..];
            let code = if from > 0 { "206 Partial Content" } else { "200 OK" };
            let _ = write!(s, "HTTP/1.1 {code}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", body.len());
            let n = if cuts > 0 { cuts -= 1; body.len() / 2 } else { body.len() };
            let _ = s.write_all(&body[..n]);
        }
    });
    Server { url: format!("{base}/go"), ranges, opens }
}
fn data() -> Vec<u8> { (0..700_000u32).map(|i| (i.wrapping_mul(2654435761) >> 13) as u8).collect() }
fn sha(b: &[u8]) -> String { Sha256::digest(b).iter().map(|b| format!("{b:02x}")).collect() }
fn dir(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("runit-dl-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
fn go(s: &Server, p: &Path, d: &[u8], h: &str) -> Result<(), String> { fetch(&s.url, p, d.len() as u64, h, &mut |_| {}, &AtomicBool::new(false)) }

#[test]
fn downloads_and_checks_a_file() {
    let d = data(); let s = serve(d.clone(), 0, false, false); let p = dir("a").join("sub/m.gguf");
    let mut seen = vec![];
    fetch(&s.url, &p, d.len() as u64, &sha(&d), &mut |n| seen.push(n), &AtomicBool::new(false)).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), d);
    assert_eq!(seen.last(), Some(&(d.len() as u64)));
    assert!(!p.with_file_name("m.gguf.part").exists());
}

#[test]
fn a_dropped_connection_resumes_through_a_fresh_link() {
    let d = data(); let s = serve(d.clone(), 2, false, false); let p = dir("b").join("m.gguf");
    go(&s, &p, &d, &sha(&d)).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), d);
    assert_eq!(s.opens.load(Ordering::SeqCst), 3);
    let r = s.ranges.lock().unwrap();
    assert!(r[1].starts_with("bytes=") && r[2].starts_with("bytes=") && r[1] != r[2], "{r:?}");
}

#[test]
fn a_partial_file_from_last_time_is_continued() {
    let d = data(); let s = serve(d.clone(), 0, false, false); let p = dir("c").join("m.gguf");
    std::fs::write(p.with_file_name("m.gguf.part"), &d[..300_000]).unwrap();
    go(&s, &p, &d, &sha(&d)).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), d);
    assert_eq!(s.ranges.lock().unwrap()[0], "bytes=300000-");
}

#[test]
fn a_server_that_ignores_range_restarts_cleanly() {
    let d = data(); let s = serve(d.clone(), 0, true, false); let p = dir("d").join("m.gguf");
    std::fs::write(p.with_file_name("m.gguf.part"), &d[..300_000]).unwrap();
    go(&s, &p, &d, &sha(&d)).unwrap();
    assert_eq!(std::fs::read(&p).unwrap(), d);
}

#[test]
fn wrong_bytes_are_thrown_away() {
    let d = data(); let s = serve(d.clone(), 0, false, false); let p = dir("e").join("m.gguf");
    let err = go(&s, &p, &d, &"0".repeat(64)).unwrap_err();
    assert!(err.contains("checksum"), "{err}");
    assert!(!p.exists() && !p.with_file_name("m.gguf.part").exists());
}

#[test]
fn a_busy_server_is_retried() {
    let d = data(); let s = serve(d.clone(), 0, false, true); let p = dir("f").join("m.gguf");
    go(&s, &p, &d, &sha(&d)).unwrap();
    assert_eq!(s.opens.load(Ordering::SeqCst), 2);
}

#[test]
fn stopping_keeps_what_was_fetched() {
    let d = data(); let s = serve(d.clone(), 0, false, false); let p = dir("g").join("m.gguf");
    let stop = AtomicBool::new(false);
    let e = fetch(&s.url, &p, d.len() as u64, &sha(&d), &mut |n| if n > 100_000 { stop.store(true, Ordering::SeqCst) }, &stop).unwrap_err();
    assert_eq!(e, "stopped");
    assert!(std::fs::metadata(p.with_file_name("m.gguf.part")).unwrap().len() > 100_000);
    assert!(!p.exists());
}

#[test]
fn remote_paths_cannot_leave_the_models_folder() {
    let root = Path::new("/models");
    assert_eq!(safe_path(root, "o/r", "BF16/m.gguf").unwrap(), root.join("o").join("r").join("BF16").join("m.gguf"));
    for bad in ["../x", "a/../../x", "/etc/x", "a\\..\\x", "C:\\x", "a//b", "", "a/.", "nul\0"] { assert!(safe_path(root, "o/r", bad).is_err(), "{bad:?}"); }
    for bad in ["../r", "o/..", "o", "o/r/x", "C:/r"] { assert!(safe_path(root, bad, "m.gguf").is_err(), "{bad:?}"); }
}
