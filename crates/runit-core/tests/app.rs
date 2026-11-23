use runit_core::{app::App, bench::{Report, Run}, engine::Device, hardware::Machine, predict::*, store};
use serde_json::json;
use std::path::PathBuf;
use std::sync::Arc;

fn dir(n: &str) -> PathBuf { let d = std::env::temp_dir().join(format!("runit-app-{n}-{}", std::process::id())); let _ = std::fs::remove_dir_all(&d); std::fs::create_dir_all(&d).unwrap(); d }
fn report() -> Report {
    let f = Fit { decode_s_per_byte: 1e-11, decode_s_per_layer: 0.0, prompt_s_per_param: 1e-12, prompt_s_per_layer: 0.0, kv_s_per_byte: 1e-11 };
    Report { machine: Machine { os: "t".into(), cpu: "Test CPU".into(), cores: 4, threads: 8, performance_cores: 4, ram_bytes: 16 << 30, ram_free_bytes: 8 << 30, gpu: Some(Device { id: "MTL0".into(), name: "M".into(), total_mib: 12124, free_mib: 12124, integrated: true }) },
        calibration: Calibration { gpu: Some(f), cpu: f, sustained: 1.0 }, conditions: Default::default(), runs: vec![Run { place: "gpu".into(), model_bytes: 1, decode: 1.0, prompt: 1.0 }], taken_at: 1, seconds: 60.0, bytes_per_s: 1e11 }
}
fn open(d: &PathBuf) -> App { App::open(d.clone(), Some("/no/engine/here".into()), Arc::new(|_| {})).unwrap() }

#[test]
fn opens_without_an_engine_and_reports_state() {
    let d = dir("a"); let a = open(&d);
    let s = a.call("state", &json!({})).unwrap();
    assert!(s["report"].is_null() && s["items"].as_array().unwrap().is_empty());
    assert!(s["models_dir"].as_str().unwrap().ends_with("models"));
    assert!(a.call("bench.start", &json!({})).unwrap_err().contains("engine"));
    assert!(a.call("recommend", &json!({})).unwrap_err().contains("test"));
}

#[test]
fn a_saved_report_is_remembered() {
    let d = dir("b"); store::save(&d.join("report.json"), &report()).unwrap();
    assert_eq!(open(&d).call("state", &json!({})).unwrap()["report"]["machine"]["cpu"], "Test CPU");
}

#[test]
fn unknown_calls_and_bad_parameters_are_plain_errors() {
    let a = open(&dir("c"));
    assert!(a.call("nope", &json!({})).is_err());
    assert!(a.call("pause", &json!({})).is_err());
    assert!(a.call("search", &json!({"q": 5})).is_err());
    assert!(a.call("chat.send", &json!({"messages": []})).is_err());
}

#[test]
fn the_models_folder_can_be_moved_and_is_remembered() {
    let d = dir("d"); let other = d.join("elsewhere");
    open(&d).call("settings.set", &json!({"models_dir": other})).unwrap();
    assert_eq!(open(&d).call("state", &json!({})).unwrap()["models_dir"], json!(other));
    assert!(other.is_dir());
    assert!(open(&d).call("settings.set", &json!({"models_dir": "relative"})).is_err());
}

#[test]
fn a_download_must_come_from_the_live_list() {
    let a = open(&dir("e"));
    // Callers name a model and files; they never supply links or checksums.
    assert!(a.call("download", &json!({"id": "x"})).is_err());
}
