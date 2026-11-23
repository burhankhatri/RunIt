use runit_core::http;
use std::io::{BufRead, BufReader, Write};
use std::net::TcpListener;
use std::sync::{atomic::{AtomicUsize, Ordering}, Arc};

/// Hangs up on the first `drops` requests, then answers.
fn flaky(drops: usize) -> (String, Arc<AtomicUsize>) {
    let l = TcpListener::bind("127.0.0.1:0").unwrap();
    let url = format!("http://{}/x", l.local_addr().unwrap());
    let seen = Arc::new(AtomicUsize::new(0));
    let s2 = seen.clone();
    std::thread::spawn(move || for s in l.incoming().flatten() {
        let mut s = s;
        for h in BufReader::new(s.try_clone().unwrap()).lines().map_while(Result::ok) { if h.is_empty() { break; } }
        if s2.fetch_add(1, Ordering::SeqCst) < drops { continue; }
        let _ = write!(s, "HTTP/1.1 200 OK\r\nContent-Length: 2\r\nConnection: close\r\n\r\nok");
    });
    (url, seen)
}

#[test]
fn a_dropped_connection_is_tried_again() {
    let (url, seen) = flaky(2);
    assert_eq!(http::text(&url).unwrap(), "ok");
    assert_eq!(seen.load(Ordering::SeqCst), 3);
}

#[test]
fn a_server_that_keeps_hanging_up_is_reported_after_a_few_tries() {
    let (url, seen) = flaky(usize::MAX);
    assert!(http::text(&url).unwrap_err().contains("could not reach"));
    assert_eq!(seen.load(Ordering::SeqCst), 3);
}

#[test]
fn nothing_listening_is_reported_quickly() {
    let t = std::time::Instant::now();
    assert!(http::text("http://127.0.0.1:9/x").is_err());
    assert!(t.elapsed().as_secs() < 10);
}
