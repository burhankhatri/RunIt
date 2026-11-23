//! `runit`: the app without a window.
use runit_core::{app::App, engine::Engine, gguf, items, predict::*, run, store, Emit};
use serde_json::{json, Value};
use std::io::{Read, Write};
use std::path::PathBuf;
use std::sync::{atomic::AtomicBool, mpsc, Arc};

const HELP: &str = "RunIt: which open models this computer runs well, and how fast.

  runit test                 test this computer (about a minute, nothing is uploaded)
  runit models               models that run well here, with predicted speed
  runit search WORDS         search Hugging Bay
  runit show ID              files and predicted speed for one model
  runit get ID [QUANT]       download a model
  runit list                 downloaded models
  runit measure KEY          time a downloaded model for real
  runit chat KEY WORDS       ask a downloaded model something
  runit remove KEY           delete a downloaded model
  runit check FILE.gguf...   predicted against measured speed for local files

Add --json for machine-readable output.";

fn main() {
    let mut args: Vec<String> = std::env::args().skip(1).collect();
    let as_json = args.iter().any(|a| a == "--json");
    args.retain(|a| a != "--json");
    if let Err(e) = go(&args, as_json) {
        eprintln!("runit: {e}");
        std::process::exit(1);
    }
}

fn gb(v: &Value) -> String {
    format!("{:.1} GB", v.as_f64().unwrap_or(0.0) / 1e9)
}

fn speed(p: &Value) -> String {
    match p["tier"].as_str() {
        None => "-".into(),
        Some("too_big") => "too big".into(),
        Some(t) => format!("{}{:.0} tok/s{}", if p["rough"] == true { "~" } else { "" }, p["tokens_per_s"].as_f64().unwrap_or(0.0), if t == "comfortable" { String::new() } else { format!(" ({t})") }),
    }
}

fn go(args: &[String], as_json: bool) -> Result<(), String> {
    let arg = |n: usize| args.get(n).map(String::as_str).ok_or_else(|| HELP.to_string());
    let (tx, rx) = mpsc::channel::<Value>();
    let emit: Emit = Arc::new(move |v| {
        let _ = tx.send(v);
    });
    let Some(cmd) = args.first() else {
        println!("{HELP}");
        return Ok(());
    };
    if cmd == "help" || cmd == "--help" || cmd == "-h" {
        println!("{HELP}");
        return Ok(());
    }
    if cmd == "--version" {
        println!("runit {}", env!("CARGO_PKG_VERSION"));
        return Ok(());
    }
    if cmd == "check" {
        return check(&args[1..], as_json);
    }
    let app = App::open(store::data_dir(), None, emit)?;
    let show = |v: &Value| println!("{}", serde_json::to_string_pretty(v).unwrap_or_default());
    match cmd.as_str() {
        "test" => {
            app.call("bench.start", &json!({}))?;
            for v in rx {
                match v["kind"].as_str() {
                    Some("finding") if !as_json => println!("{}", v["text"].as_str().unwrap_or("")),
                    Some("done") => {
                        if as_json {
                            show(&v["report"]);
                        }
                        return Ok(());
                    }
                    Some("failed") => return Err(v["error"].as_str().unwrap_or("failed").into()),
                    _ => {}
                }
            }
        }
        "models" => {
            let v = app.call("recommend", &json!({}))?;
            if as_json {
                show(&v);
                return Ok(());
            }
            for p in v["picks"].as_array().into_iter().flatten() {
                let c = &p["choice"];
                println!("{}: {}\n  {}  {}  {}\n  {}\n  runit get {} {}\n", p["title"].as_str().unwrap_or(""), c["name"].as_str().unwrap_or(""), speed(&c["prediction"]), gb(&c["variant"]["size"]), c["variant"]["label"].as_str().unwrap_or(""), p["why"].as_str().unwrap_or(""), c["id"].as_str().unwrap_or(""), c["variant"]["label"].as_str().unwrap_or(""));
            }
            println!("{:<34} {:>6} {:>18} {:>9}  quant", "model", "rating", "speed", "size");
            for c in v["ranked"].as_array().into_iter().flatten() {
                println!("{:<34} {:>6} {:>18} {:>9}  {}", c["name"].as_str().unwrap_or(""), c["rating"]["rating"], speed(&c["prediction"]), gb(&c["variant"]["size"]), c["variant"]["label"].as_str().unwrap_or(""));
            }
            println!("\nPopular, not rated yet");
            for c in v["unrated"].as_array().into_iter().flatten() {
                println!("{:<34} {:>6} {:>18} {:>9}  {}", c["name"].as_str().unwrap_or(""), "-", speed(&c["prediction"]), gb(&c["variant"]["size"]), c["variant"]["label"].as_str().unwrap_or(""));
            }
            println!("\n{} of {}", v["ratings_source"].as_str().unwrap_or(""), v["ratings_date"].as_str().unwrap_or(""));
        }
        "search" => {
            let v = app.call("search", &json!({"q": args[1..].join(" ")}))?;
            if as_json {
                show(&v);
                return Ok(());
            }
            for h in v["hits"].as_array().into_iter().flatten() {
                println!("{:<60} {:>12} downloads  {}", h["repo"].as_str().unwrap_or(""), h["downloads"], h["id"].as_str().unwrap_or(""));
            }
        }
        "show" => {
            let v = app.call("model", &json!({"id": arg(1)?}))?;
            if as_json {
                show(&v);
                return Ok(());
            }
            println!("{}  {}", v["repo"].as_str().unwrap_or(""), v["license"].as_str().unwrap_or(""));
            for x in v["variants"].as_array().into_iter().flatten() {
                println!("  {:<10} {:>9}  {}", x["variant"]["label"].as_str().unwrap_or(""), gb(&x["variant"]["size"]), speed(&x["prediction"]));
            }
            for f in v["others"].as_array().into_iter().flatten() {
                println!("  {:>9}  {}", gb(&f["size"]), f["path"].as_str().unwrap_or(""));
            }
        }
        "get" => {
            let v = app.call("model", &json!({"id": arg(1)?}))?;
            let want = args.get(2).map(String::as_str);
            let all = v["variants"].as_array().cloned().unwrap_or_default();
            let pick = all
                .iter()
                .find(|x| x["variant"]["label"].as_str() == Some(want.unwrap_or("Q4_K_M")))
                .or(if want.is_none() { all.first() } else { None })
                .ok_or("no such file. See `runit show`")?;
            let paths: Vec<_> = pick["variant"]["files"].as_array().into_iter().flatten().map(|f| f["path"].clone()).collect();
            let key = app.call("download", &json!({"id": arg(1)?, "paths": paths}))?["key"].as_str().unwrap_or("").to_string();
            for v in rx {
                let i = &v["item"];
                if i["key"] != key.as_str() {
                    continue;
                }
                print!("\r{} of {}  {}/s   ", gb(&i["done"]), gb(&i["total"]), gb(&i["bytes_per_s"]));
                let _ = std::io::stdout().flush();
                match i["status"].as_str() {
                    Some("ready") => {
                        println!("\n{key}");
                        return Ok(());
                    }
                    Some("failed") => return Err(format!("\n{}", i["error"].as_str().unwrap_or("failed"))),
                    _ => {}
                }
            }
        }
        "list" => {
            let v = app.call("state", &json!({}))?;
            if as_json {
                show(&v["items"]);
                return Ok(());
            }
            for i in v["items"].as_array().into_iter().flatten() {
                println!("{:<8} {:>9}  {}", i["status"].as_str().unwrap_or(""), gb(&i["total"]), i["key"].as_str().unwrap_or(""));
            }
        }
        "measure" => {
            app.call("measure", &json!({"key": arg(1)?}))?;
            for v in rx {
                match (v["type"].as_str(), v["kind"].as_str()) {
                    (Some("measure"), Some("done")) => {
                        println!("{:.1} tokens/s, reading at {:.0} tokens/s", v["measured"]["tokens_per_s"].as_f64().unwrap_or(0.0), v["measured"]["prompt_tokens_per_s"].as_f64().unwrap_or(0.0));
                        return Ok(());
                    }
                    (Some("measure"), Some("failed")) => return Err(v["error"].as_str().unwrap_or("failed").into()),
                    _ => {}
                }
            }
        }
        "chat" => {
            app.call("chat.open", &json!({"key": arg(1)?}))?;
            app.call("chat.send", &json!({"messages": [{"role": "user", "content": args[2..].join(" ")}]}))?;
            let mut last = 0.0;
            for v in rx {
                match v["kind"].as_str() {
                    Some("text" | "thought") => {
                        print!("{}", v["text"].as_str().unwrap_or(""));
                        let _ = std::io::stdout().flush();
                    }
                    Some("speed") => last = v["tokens_per_s"].as_f64().unwrap_or(0.0),
                    Some("end") => {
                        println!("\n[{last:.1} tokens/s]");
                        return Ok(());
                    }
                    Some("failed") => return Err(v["error"].as_str().unwrap_or("failed").into()),
                    _ => {}
                }
            }
        }
        "remove" => {
            app.call("remove", &json!({"key": arg(1)?}))?;
        }
        _ => return Err(HELP.into()),
    }
    Ok(())
}

/// Predict local files from their headers, then time them.
fn check(files: &[String], as_json: bool) -> Result<(), String> {
    let report: runit_core::bench::Report = store::load(&store::data_dir().join("report.json")).ok_or("run `runit test` first")?;
    let engine = Engine::locate()?;
    let place = match (&report.calibration.gpu, &report.machine.gpu) {
        (Some(_), Some(g)) => runit_core::engine::Place::Gpu(g.id.clone()),
        _ => runit_core::engine::Place::Cpu,
    };
    let mut rows = vec![];
    for f in files {
        let path = PathBuf::from(f).canonicalize().map_err(|e| format!("{f}: {e}"))?;
        let size = path.metadata().map_err(|e| format!("{f}: {e}"))?.len();
        // The same slice of the file the app reads over the network.
        let mut head = vec![];
        std::fs::File::open(&path).and_then(|h| h.take(96 << 10).read_to_end(&mut head)).map_err(|e| e.to_string())?;
        let shape = gguf::read_header(&head)?;
        let p = predict(&load_from_shape(&shape, size), &report.machine, &report.calibration);
        let m: items::Measured = run::measure(&engine, &path, &place, 99, &AtomicBool::new(false))?;
        let off = (p.tokens_per_s / m.tokens_per_s - 1.0) * 100.0;
        if !as_json {
            println!("{:<44} predicted {:>6.1}  measured {:>6.1}  {:+5.1}%   prompt {:>6.0} / {:>6.0}", path.file_name().unwrap_or_default().to_string_lossy(), p.tokens_per_s, m.tokens_per_s, off, p.prompt_tokens_per_s, m.prompt_tokens_per_s);
        }
        rows.push(json!({"file": f, "predicted": p.tokens_per_s, "measured": m.tokens_per_s, "error_percent": off, "prompt_predicted": p.prompt_tokens_per_s, "prompt_measured": m.prompt_tokens_per_s}));
    }
    if as_json {
        println!("{}", serde_json::to_string_pretty(&rows).unwrap_or_default());
    }
    Ok(())
}
