//! Run a downloaded model: measure it, or talk to it.
use crate::conditions::Conditions;
use crate::engine::{Engine, Guard, Place};
use crate::items::Measured;
use serde::Deserialize;
use serde_json::{json, Value};
use std::io::{BufRead, BufReader};
use std::net::TcpListener;
use std::path::Path;
use std::process::Stdio;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

/// Time a real model. `gpu_layers` of 99 means all of them.
pub fn measure(e: &Engine, model: &Path, place: &Place, gpu_layers: u32, stop: &AtomicBool) -> Result<Measured, String> {
    let before = Conditions::now();
    let rows = e.bench_layers(model, place, gpu_layers, &["-p", "256", "-n", "64", "-r", "3"], stop)?;
    let find = |gen: bool| rows.iter().find(|r| (r.generate > 0) == gen).map(|r| r.tokens_per_s).ok_or("the engine gave no timing");
    let after = Conditions::now();
    Ok(Measured { tokens_per_s: find(true)?, prompt_tokens_per_s: find(false)?, conditions: Conditions { hot: before.hot || after.hot, ..after } })
}

#[derive(Debug, PartialEq)]
pub enum Piece {
    Text(String),
    Thought(String),
    Speed { tokens_per_s: f64, prompt_tokens_per_s: f64 },
    End,
}

/// One line of a streamed reply.
pub fn parse_event(line: &str) -> Vec<Piece> {
    let Some(data) = line.strip_prefix("data:").map(str::trim) else { return vec![] };
    if data == "[DONE]" {
        return vec![Piece::End];
    }
    let Ok(v) = serde_json::from_str::<Value>(data) else { return vec![] };
    let delta = &v["choices"][0]["delta"];
    let said = |k: &str| delta[k].as_str().filter(|s| !s.is_empty()).map(str::to_string);
    let mut out = Vec::new();
    out.extend(said("reasoning_content").map(Piece::Thought));
    out.extend(said("content").map(Piece::Text));
    if let Some(t) = v["timings"]["predicted_per_second"].as_f64() {
        out.push(Piece::Speed { tokens_per_s: t, prompt_tokens_per_s: v["timings"]["prompt_per_second"].as_f64().unwrap_or(0.0) });
    }
    out
}

#[derive(Clone, Debug, Deserialize)]
pub struct Message {
    pub role: String,
    pub content: String,
}

/// A loaded model, listening only on this computer behind a one-time key.
pub struct Server {
    _child: Guard,
    base: String,
    key: String,
}

impl Server {
    pub fn start(e: &Engine, model: &Path, place: &Place, stop: &AtomicBool) -> Result<Server, String> {
        let port = TcpListener::bind("127.0.0.1:0").and_then(|l| l.local_addr()).map_err(|e| e.to_string())?.port();
        let mut raw = [0u8; 24];
        getrandom::fill(&mut raw).map_err(|e| e.to_string())?;
        let key = crate::catalog::hex(&raw);
        let mut c = e.command("llama-server");
        c.arg("-m").arg(model).args(["--host", "127.0.0.1", "--port", &port.to_string(), "-c", "8192", "-np", "1", "--no-ui"]);
        match place {
            Place::Gpu(id) => c.args(["-dev", id]),
            Place::Cpu => c.args(["-dev", "none", "-ngl", "0"]),
        };
        // Through the environment, so it does not show in the process list.
        c.env("LLAMA_API_KEY", &key).stdout(Stdio::null()).stderr(Stdio::null());
        let mut child = Guard::spawn(c).map_err(|e| format!("could not start the model: {e}"))?;
        let base = format!("http://127.0.0.1:{port}");
        let start = Instant::now();
        loop {
            if stop.load(Ordering::Relaxed) {
                return Err("stopped".into());
            }
            if let Ok(Some(_)) = child.ended() {
                return Err("the model could not be loaded. It may need more memory than is free, or a newer engine".into());
            }
            if local().get(format!("{base}/health")).call().is_ok_and(|r| r.status() == 200) {
                return Ok(Server { _child: child, base, key });
            }
            if start.elapsed() > Duration::from_secs(180) {
                return Err("the model took too long to load".into());
            }
            std::thread::sleep(Duration::from_millis(100));
        }
    }

    pub fn chat(&self, messages: &[Message], on: &mut dyn FnMut(Piece), stop: &AtomicBool) -> Result<(), String> {
        let body = json!({
            "messages": messages.iter().map(|m| json!({"role": m.role, "content": m.content})).collect::<Vec<_>>(),
            "stream": true,
            "timings_per_token": true,
            "max_tokens": 4096,
        });
        let r = local()
            .post(format!("{}/v1/chat/completions", self.base))
            .header("Authorization", format!("Bearer {}", self.key))
            .header("Content-Type", "application/json")
            .send(body.to_string())
            .map_err(|e| format!("the model stopped answering: {e}"))?;
        if r.status() != 200 {
            return Err(match r.status().as_u16() {
                400 => "this conversation no longer fits. Start a new one".into(),
                s => format!("the model answered with an error ({s})"),
            });
        }
        for line in BufReader::new(r.into_body().into_reader()).lines() {
            if stop.load(Ordering::Relaxed) {
                return Ok(());
            }
            for p in parse_event(&line.map_err(|e| format!("the model stopped answering: {e}"))?) {
                if p == Piece::End {
                    return Ok(());
                }
                on(p);
            }
        }
        Ok(())
    }
}

fn local() -> ureq::Agent {
    ureq::Agent::config_builder().http_status_as_error(false).proxy(None).build().into()
}
