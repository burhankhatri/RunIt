//! Time the real engine on generated models and fit a speed curve.
use crate::conditions::Conditions;
use crate::engine::{BenchRow, Engine, Place};
use crate::gguf::{read_header, write_synthetic, Synthetic};
use crate::hardware::{disk_free, Machine};
use crate::predict::{fit, load_from_shape, Calibration, Load, Sample};
use crate::Emit;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

pub const SMALL: Synthetic = Synthetic { layers: 16, embedding: 1024, feed_forward: 4096, heads: 16, kv_heads: 4, vocab: 32000 };
pub const MEDIUM: Synthetic = Synthetic { layers: 28, embedding: 1536, feed_forward: 8960, heads: 12, kv_heads: 2, vocab: 64000 };
pub const LARGE: Synthetic = Synthetic { layers: 28, embedding: 3072, feed_forward: 8192, heads: 24, kv_heads: 8, vocab: 64000 };

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Run { pub place: String, pub model_bytes: u64, pub decode: f64, pub prompt: f64 }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Report {
    pub machine: Machine,
    pub calibration: Calibration,
    /// What was holding the computer back when the test began.
    #[serde(default)]
    pub conditions: Conditions,
    pub runs: Vec<Run>,
    pub taken_at: u64,
    pub seconds: f64,
    /// Weight bytes read per second while generating, on the main device.
    pub bytes_per_s: f64,
}

const GB: f64 = 1e9;

fn file_size(s: &Synthetic) -> u64 {
    let (emb, ff) = (s.embedding as u64, s.feed_forward as u64);
    let kv = emb / s.heads as u64 * s.kv_heads as u64;
    let layer = 2 * emb * emb + 2 * emb * kv + 3 * emb * ff;
    (s.layers as u64 * layer + emb * s.vocab as u64) / 256 * 144 + emb * s.vocab as u64 / 256 * 210
}

/// Test models this machine has the memory and disk for. Each is deleted before the next is made.
pub fn sizes(capacity: u64, disk: Option<u64>) -> Vec<Synthetic> {
    [SMALL, MEDIUM, LARGE]
        .into_iter()
        .filter(|s| {
            let size = file_size(s);
            size * 3 / 2 < capacity && disk.is_none_or(|d| size + 500_000_000 < d)
        })
        .collect()
}

/// Deletes the file when dropped, on every way out.
struct Temp(PathBuf);

impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

pub fn run(e: &Engine, dir: &Path, emit: &Emit, stop: &AtomicBool) -> Result<Report, String> {
    let started = Instant::now();
    let finding = |key: &str, text: String, progress: f64| emit(json!({"type": "bench", "kind": "finding", "key": key, "text": text, "progress": progress}));
    let stage = |key: &str, text: &str, progress: f64| emit(json!({"type": "bench", "kind": "stage", "key": key, "text": text, "progress": progress}));
    let check = || if stop.load(Ordering::Relaxed) { Err("stopped".to_string()) } else { Ok(()) };

    stage("machine", "Looking at the hardware", 0.0);
    let mut m = Machine::detect(e);
    finding("cpu", cpu_line(&m), 0.03);
    check()?;
    finding("memory", memory_line(&m), 0.06);
    check()?;
    let conditions = Conditions::now();
    if let Some(note) = conditions.note() {
        finding("conditions", note.into(), 0.07);
    }

    std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    // Left over from a crash or a power cut.
    for f in std::fs::read_dir(dir).map_err(|e| e.to_string())?.flatten() {
        if f.path().extension().is_some_and(|x| x == "gguf") {
            let _ = std::fs::remove_file(f.path());
        }
    }
    let disk = disk_free(dir);
    let plan = sizes(m.capacity(), disk);
    if plan.len() < 2 {
        return Err(format!("the test needs about 1.6 GB of free disk and 2 GB of free memory; this disk has {:.1} GB free", disk.unwrap_or(0) as f64 / GB));
    }

    let mut gpu = m.gpu.as_ref().map(|g| Place::Gpu(g.id.clone()));
    let mut runs = Vec::new();
    let (mut on_gpu, mut on_cpu) = (Vec::new(), Vec::new());
    let mut deep = None;
    let mut held = 1.0;
    let make = |i: usize, at: f64| -> Result<(Temp, Load), String> {
        check()?;
        stage("write", "Building a test model with random weights", at);
        let file = Temp(dir.join(format!("test-{i}.gguf")));
        let bytes = write_synthetic(&file.0, &plan[i]).map_err(|e| format!("could not write the test model: {e}"))?;
        check()?;
        let mut head = vec![0; 256 << 10];
        let n = std::fs::File::open(&file.0).and_then(|mut f| f.read(&mut head)).map_err(|e| e.to_string())?;
        Ok((file, load_from_shape(&read_header(&head[..n])?, bytes)))
    };

    // Graphics first, while the machine is cool, smallest to largest.
    for i in 0..plan.len() {
        let Some(place) = gpu.clone() else { break };
        let at = 0.08 + 0.5 * i as f64 / plan.len() as f64;
        let (file, load) = make(i, at)?;
        if i == 0 {
            stage("warm", "Waking the graphics processor", at + 0.02);
            // Compiles shaders the first time, which must not count as speed.
            if let Err(err) = e.bench(&file.0, &place, &["-p", "32", "-n", "4", "-r", "1"], stop) {
                check()?;
                finding("gpu", format!("The graphics processor could not run the test, so this uses the processor. ({err})"), at + 0.03);
                (gpu, m.gpu) = (None, None);
                break;
            }
        }
        stage("gpu", "Timing it on the graphics processor", at + 0.05);
        let s = sample(&load, &e.bench(&file.0, &place, &["-p", "512", "-n", "48", "-r", "4"], stop)?)?;
        runs.push(record("gpu", load.file_bytes, &s));
        finding("gpu-run", run_line(&load, &s, "graphics processor"), at + 0.12);
        on_gpu.push(s);
        if i == 1 {
            // How much slower a long conversation gets.
            let r = e.bench(&file.0, &place, &["-p", "0", "-n", "32", "-d", "2048", "-r", "3"], stop)?;
            deep = r.iter().find(|r| r.generate > 0).map(|r| (s, load.kv_bytes_per_token * 2048.0, 1.0 / r.tokens_per_s));
        }
        if i + 1 == plan.len() {
            stage("steady", "Seeing how it holds up when it gets warm", 0.6);
            held = steady(e, &file.0, &place, 1.0 / s.decode_s, stop)?;
            finding("steady", steady_line(held), 0.75);
        }
    }

    // A processor alone also runs the large one; otherwise it only backs up the graphics processor.
    let count = if gpu.is_some() { 2 } else { plan.len() };
    for i in 0..count {
        let at = 0.76 + 0.2 * i as f64 / count as f64;
        let (file, load) = make(i, at)?;
        stage("cpu", "Timing it on the processor", at + 0.01);
        let s = sample(&load, &e.bench(&file.0, &Place::Cpu, &["-p", "64", "-n", "16", "-r", "3"], stop)?)?;
        runs.push(record("cpu", load.file_bytes, &s));
        finding("cpu-run", run_line(&load, &s, "processor"), at + 0.06);
        on_cpu.push(s);
        if i + 1 == count && gpu.is_some() && processor_is_quicker(&on_gpu[i], &s) {
            (gpu, m.gpu) = (None, None);
            finding("gpu", "The processor is quicker than the graphics here, so models will run on it.".into(), 0.965);
        }
        if gpu.is_none() && i + 1 == count {
            held = steady(e, &file.0, &Place::Cpu, 1.0 / s.decode_s, stop)?;
            finding("steady", steady_line(held), 0.97);
        }
    }

    stage("fit", "Fitting the speed curve", 0.98);
    let cpu = fit(&on_cpu, None).ok_or("the timings were unusable; close heavy apps and test again")?;
    let gpu = match gpu {
        Some(_) => Some(fit(&on_gpu, deep).ok_or("the graphics timings were unusable; close heavy apps and test again")?),
        None => None,
    };
    let bytes_per_s = 1.0 / gpu.as_ref().unwrap_or(&cpu).decode_s_per_byte;
    finding("bandwidth", format!("It reads {:.0} GB of model every second while it writes.", bytes_per_s / GB), 1.0);
    Ok(Report {
        machine: m,
        calibration: Calibration { gpu, cpu, sustained: held },
        conditions,
        runs,
        taken_at: SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_secs()),
        seconds: started.elapsed().as_secs_f64(),
        bytes_per_s,
    })
}

/// End of a run as a share of its peak, at most 1.
pub fn sustained(samples: &[f64]) -> f64 {
    let Some(peak) = samples.iter().copied().reduce(f64::max).filter(|p| *p > 0.0) else { return 1.0 };
    let end = &samples[samples.len().saturating_sub(3)..];
    (end.iter().sum::<f64>() / end.len() as f64 / peak).min(1.0)
}

/// Whether models should run on the processor although graphics work.
pub fn processor_is_quicker(graphics: &Sample, processor: &Sample) -> bool {
    processor.decode_s * 1.15 < graphics.decode_s
}

/// About twenty seconds of steady writing.
fn steady(e: &Engine, file: &Path, place: &Place, speed: f64, stop: &AtomicBool) -> Result<f64, String> {
    let n = (speed * 2.0).clamp(8.0, 256.0) as u32;
    let rows = e.bench(file, place, &["-p", "0", "-n", &n.to_string(), "-r", "10"], stop)?;
    Ok(rows.first().map_or(1.0, |r| sustained(&r.samples)))
}

fn steady_line(held: f64) -> String {
    if held > 0.9 {
        "Twenty seconds in, it holds that speed.".into()
    } else {
        format!("Twenty seconds in, it has slowed to {:.0}% of that.", held * 100.0)
    }
}

fn sample(l: &Load, rows: &[BenchRow]) -> Result<Sample, String> {
    let prompt = rows.iter().find(|r| r.prompt > 0 && r.generate == 0).ok_or("no prompt timing")?;
    let decode = rows.iter().find(|r| r.generate > 0).ok_or("no writing timing")?;
    Ok(Sample {
        touched_bytes: l.touched_bytes,
        active_params: l.active_params,
        layers: l.layers,
        decode_s: 1.0 / decode.tokens_per_s,
        prompt_s: 1.0 / prompt.tokens_per_s,
    })
}

fn record(place: &str, model_bytes: u64, s: &Sample) -> Run {
    Run { place: place.into(), model_bytes, decode: 1.0 / s.decode_s, prompt: 1.0 / s.prompt_s }
}

fn cpu_line(m: &Machine) -> String {
    if m.performance_cores < m.cores {
        format!("{}. {} fast cores, {} efficient ones.", m.cpu, m.performance_cores, m.cores - m.performance_cores)
    } else {
        format!("{}. {} cores.", m.cpu, m.cores)
    }
}

fn memory_line(m: &Machine) -> String {
    let ram = m.ram_bytes as f64 / (1u64 << 30) as f64;
    match &m.gpu {
        Some(g) if g.integrated => format!("{ram:.0} GB of memory, about {:.0} GB of it free for a model.", m.capacity() as f64 / (1u64 << 30) as f64),
        Some(g) => format!("{ram:.0} GB of memory, and {} with {:.1} GB free.", g.name, g.free_mib as f64 / 1024.0),
        None => format!("{ram:.0} GB of memory and no graphics processor the engine can use."),
    }
}

fn run_line(l: &Load, s: &Sample, place: &str) -> String {
    format!("{:.1} GB model, {place}: {:.0} tokens a second.", l.file_bytes as f64 / GB, 1.0 / s.decode_s)
}
