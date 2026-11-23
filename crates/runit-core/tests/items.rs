use runit_core::{catalog::FileRow, items::*};
use sha2::{Digest, Sha256};
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

fn sha(b: &[u8]) -> String { Sha256::digest(b).iter().map(|b| format!("{b:02x}")).collect() }
fn data() -> Vec<u8> { (0..400_000u32).map(|i| (i * 7 >> 3) as u8).collect() }
/// Serves `data`, pausing after the first 100 KB when `slow`.
fn serve(slow: bool) -> String {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/f", l.local_addr().unwrap());
    std::thread::spawn(move || for s in l.incoming().flatten() { std::thread::spawn(move || {
        let mut s = s; let mut from = 0;
        for h in BufReader::new(s.try_clone().unwrap()).lines().map_while(Result::ok) { if h.is_empty() { break; } if let Some(v) = h.to_lowercase().strip_prefix("range: bytes=") { from = v.trim_end_matches('-').parse().unwrap(); } }
        let d = data(); let d = &d[from..];
        let _ = write!(s, "HTTP/1.1 {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n", if from > 0 { "206 P" } else { "200 OK" }, d.len());
        let n = d.len().min(100_000); let _ = s.write_all(&d[..n]); let _ = s.flush();
        if slow && from == 0 { std::thread::sleep(Duration::from_secs(3)); }
        let _ = s.write_all(&d[n..]);
    }); });
    url
}
fn dir(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("runit-it-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); d }
fn row(url: &str, name: &str) -> FileRow { FileRow { path: name.into(), size: data().len() as u64, sha256: sha(&data()), url: url.into() } }
fn open(d: &PathBuf) -> Items { Items::open(d.join("data"), d.join("models"), Arc::new(|_| {}), Arc::new(|_| Err("offline".into()))) }
fn wait(i: &Items, key: &str, s: Status) -> Item {
    let t = Instant::now();
    loop {
        if let Some(x) = i.list().into_iter().find(|x| x.key == key && x.status == s) { return x; }
        assert!(t.elapsed() < Duration::from_secs(20), "still {:?}", i.list());
        std::thread::sleep(Duration::from_millis(20));
    }
}

#[test]
fn a_download_finishes_and_survives_a_restart() {
    let d = dir("a"); let i = open(&d); let u = serve(false);
    let it = item("id", "o/r", "R", "Q4_K_M", vec![row(&u, "a.gguf"), row(&u, "b.gguf")], None).unwrap();
    assert_eq!((it.total, it.status), (800_000, Status::Queued));
    i.add(it.clone()).unwrap();
    let r = wait(&i, &it.key, Status::Ready);
    assert_eq!(r.done, 800_000);
    assert_eq!(std::fs::read(d.join("models/o/r/b.gguf")).unwrap(), data());
    assert_eq!(i.path(&it.key).unwrap(), d.join("models/o/r/a.gguf"));
    i.set_measured(&it.key, Measured { tokens_per_s: 42.0, prompt_tokens_per_s: 300.0, conditions: Default::default() });
    drop(i);
    let again = open(&d);
    assert_eq!(again.list()[0].status, Status::Ready);
    assert_eq!(again.list()[0].measured.unwrap().tokens_per_s, 42.0);
    assert!(again.add(it).unwrap_err().contains("already"));
}

#[test]
fn pausing_keeps_bytes_and_resuming_finishes() {
    let d = dir("b"); let i = open(&d); let u = serve(true);
    let it = item("id", "o/r", "R", "Q4", vec![row(&u, "a.gguf")], None).unwrap();
    i.add(it.clone()).unwrap();
    let t = Instant::now();
    while i.list()[0].done == 0 { assert!(t.elapsed() < Duration::from_secs(10)); std::thread::sleep(Duration::from_millis(10)); }
    i.pause(&it.key).unwrap();
    wait(&i, &it.key, Status::Paused);
    assert!(d.join("models/o/r/a.gguf.part").exists());
    i.resume(&it.key).unwrap();
    wait(&i, &it.key, Status::Ready);
}

#[test]
fn an_interrupted_download_comes_back_paused() {
    let d = dir("c"); let u = serve(true);
    { let i = open(&d); i.add(item("id", "o/r", "R", "Q4", vec![row(&u, "a.gguf")], None).unwrap()).unwrap(); std::thread::sleep(Duration::from_millis(300)); }
    let s = open(&d).list()[0].status;
    assert_eq!(s, Status::Paused);
}

#[test]
fn removing_deletes_files_and_tidies_empty_folders() {
    let d = dir("d"); let i = open(&d); let u = serve(false);
    let it = item("id", "o/r", "R", "Q4", vec![row(&u, "x/a.gguf")], None).unwrap();
    i.add(it.clone()).unwrap(); wait(&i, &it.key, Status::Ready);
    i.remove(&it.key).unwrap();
    assert!(i.list().is_empty() && !d.join("models/o").exists() && d.join("models").exists());
}

#[test]
fn removing_mid_download_leaves_nothing() {
    let d = dir("e"); let i = open(&d); let u = serve(true);
    let it = item("id", "o/r", "R", "Q4", vec![row(&u, "a.gguf")], None).unwrap();
    i.add(it.clone()).unwrap(); std::thread::sleep(Duration::from_millis(300));
    i.remove(&it.key).unwrap();
    std::thread::sleep(Duration::from_millis(500));
    assert!(i.list().is_empty() && !d.join("models/o").exists());
}

#[test]
fn a_bad_file_fails_with_a_reason_and_can_be_retried() {
    let d = dir("f"); let i = open(&d); let u = serve(false);
    let it = item("id", "o/r", "R", "Q4", vec![FileRow { sha256: "0".repeat(64), ..row(&u, "a.gguf") }], None).unwrap();
    i.add(it.clone()).unwrap();
    assert!(wait(&i, &it.key, Status::Failed).error.unwrap().contains("checksum"));
    i.resume(&it.key).unwrap();
    wait(&i, &it.key, Status::Failed);
}

#[test]
fn the_live_file_list_corrects_a_stale_checksum() {
    let d = dir("g"); let u = serve(false); let fresh = row(&u, "a.gguf");
    let i = Items::open(d.join("data"), d.join("models"), Arc::new(|_| {}), Arc::new(move |_| Ok(vec![fresh.clone()])));
    let it = item("id", "o/r", "R", "Q4", vec![FileRow { sha256: "0".repeat(64), ..row(&u, "a.gguf") }], None).unwrap();
    i.add(it.clone()).unwrap();
    wait(&i, &it.key, Status::Ready);
}

#[test]
fn too_little_disk_is_refused_before_starting() {
    let d = dir("h"); let i = open(&d);
    let it = item("id", "o/r", "R", "Q4", vec![FileRow { size: 1 << 60, ..row("http://x", "a.gguf") }], None).unwrap();
    assert!(i.add(it).unwrap_err().contains("disk"));
    assert!(item("id", "o/r", "R", "Q4", vec![], None).is_err());
    assert!(item("id", "o/r", "R", "Q4", vec![row("http://x", "../a.gguf")], None).is_err());
}

#[test]
fn progress_is_announced() {
    let d = dir("i"); let u = serve(false); let seen = Arc::new(Mutex::new(vec![])); let s2 = seen.clone();
    let i = Items::open(d.join("data"), d.join("models"), Arc::new(move |v| s2.lock().unwrap().push(v)), Arc::new(|_| Err("x".into())));
    let it = item("id", "o/r", "R", "Q4", vec![row(&u, "a.gguf")], None).unwrap();
    i.add(it.clone()).unwrap(); wait(&i, &it.key, Status::Ready);
    std::thread::sleep(Duration::from_millis(50));
    let seen = seen.lock().unwrap();
    assert!(seen.iter().all(|v| v["type"] == "item") && seen.last().unwrap()["item"]["status"] == "ready");
}
