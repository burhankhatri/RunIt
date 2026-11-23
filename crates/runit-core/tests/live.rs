//! Runs the real engine. `RUNIT_ENGINE_DIR=... cargo test --features live --test live`
use runit_core::{bench, engine::Engine, predict::*};
use std::sync::{atomic::{AtomicBool, Ordering}, Arc, Mutex};

#[test]
fn a_full_test_yields_a_usable_calibration_and_cleans_up() {
    let dir = std::env::temp_dir().join(format!("runit-live-{}", std::process::id()));
    let seen = Arc::new(Mutex::new(vec![]));
    let sink = seen.clone();
    let emit: runit_core::Emit = Arc::new(move |v| sink.lock().unwrap().push(v));
    let r = bench::run(&Engine::locate().unwrap(), &dir, &emit, &AtomicBool::new(false)).unwrap();
    assert!(r.machine.ram_bytes > 1 << 30 && !r.machine.cpu.is_empty());
    assert!(r.bytes_per_s > 1e9, "{}", r.bytes_per_s);
    let p = predict(&load_from_size(700_000_000), &r.machine, &r.calibration);
    assert!(p.tokens_per_s > 0.5 && p.prompt_tokens_per_s > p.tokens_per_s * 0.5);
    assert_eq!(std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0), 0, "test models left behind");
    let seen = seen.lock().unwrap();
    assert!(seen.iter().any(|v| v["kind"] == "finding"));
    let progress: Vec<f64> = seen.iter().filter_map(|v| v["progress"].as_f64()).collect();
    assert!(progress.windows(2).all(|w| w[0] <= w[1]), "progress went backwards: {progress:?}");
    assert_eq!(progress.last(), Some(&1.0));
}

#[test]
fn a_test_can_be_stopped() {
    let dir = std::env::temp_dir().join(format!("runit-stop-{}", std::process::id()));
    let stop = Arc::new(AtomicBool::new(false));
    let s2 = stop.clone();
    let emit: runit_core::Emit = Arc::new(move |v| if v["kind"] == "finding" { s2.store(true, Ordering::Relaxed) });
    assert_eq!(bench::run(&Engine::locate().unwrap(), &dir, &emit, &stop).unwrap_err(), "stopped");
    assert_eq!(std::fs::read_dir(&dir).map(|d| d.count()).unwrap_or(0), 0);
}
