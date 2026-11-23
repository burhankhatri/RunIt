//! The pinned llama.cpp binaries, run as child processes.
use serde::{Deserialize, Serialize};
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Device {
    pub id: String,
    pub name: String,
    pub total_mib: u64,
    pub free_mib: u64,
    /// Shares system memory.
    pub integrated: bool,
}

fn is_integrated(id: &str, name: &str) -> bool {
    if id.starts_with("MTL") {
        return true;
    }
    let n = name.to_lowercase();
    if n.contains("nvidia") || n.contains("geforce") {
        return false;
    }
    if n.contains("radeon") {
        // Discrete parts are named RX, Pro W, VII, Instinct. Plain "Radeon Graphics" and 780M are not.
        return !(n.contains(" rx ") || n.contains("pro w") || n.contains("radeon vii"));
    }
    if n.contains("arc") {
        // A580, B580, A770 are cards. 140V and plain "Arc Graphics" live in the processor.
        let card = n.split(|c: char| !c.is_alphanumeric()).any(|w| {
            let b = w.as_bytes();
            b.len() >= 4 && (b[0] == b'a' || b[0] == b'b') && b[1..4].iter().all(u8::is_ascii_digit)
        });
        return !card;
    }
    true
}

/// Reads `llama-bench --list-devices`.
pub fn parse_devices(out: &str) -> Vec<Device> {
    out.lines()
        .skip_while(|l| !l.starts_with("Available devices"))
        .skip(1)
        .filter_map(|l| {
            let (id, rest) = l.trim().split_once(": ")?;
            let open = rest.rfind(" (")?;
            let (name, mem) = (rest[..open].trim(), &rest[open + 2..]);
            let mut nums = mem.split(|c: char| !c.is_ascii_digit()).filter(|s| !s.is_empty());
            let total_mib: u64 = nums.next()?.parse().ok()?;
            let free_mib: u64 = nums.next()?.parse().ok()?;
            (total_mib > 0).then(|| Device {
                integrated: is_integrated(id, name),
                id: id.into(),
                name: name.into(),
                total_mib,
                free_mib,
            })
        })
        .collect()
}

pub fn best_device(d: &[Device]) -> Option<&Device> {
    d.iter().max_by_key(|d| (!d.integrated, d.total_mib))
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct BenchRow {
    pub prompt: u32,
    pub generate: u32,
    pub depth: u32,
    /// The best sample: other work can only slow a run down.
    pub tokens_per_s: f64,
    /// Every sample in order.
    pub samples: Vec<f64>,
    pub model_bytes: u64,
}

/// Reads `llama-bench -o json`.
pub fn parse_bench(out: &str) -> Result<Vec<BenchRow>, String> {
    #[derive(Deserialize)]
    struct Raw {
        n_prompt: u32,
        n_gen: u32,
        #[serde(default)]
        n_depth: u32,
        avg_ts: f64,
        #[serde(default)]
        samples_ts: Vec<f64>,
        model_size: u64,
    }
    let at = out.lines().scan(0, |o, l| { let s = *o; *o += l.len() + 1; Some((s, l)) }).find(|(_, l)| l.trim_start().starts_with('[')).map(|(s, _)| s);
    let raw: Vec<Raw> = serde_json::from_str(&out[at.ok_or("the engine printed no results")?..]).map_err(|e| format!("unreadable engine results: {e}"))?;
    if raw.is_empty() {
        return Err("the engine printed no results".into());
    }
    Ok(raw
        .into_iter()
        .map(|r| BenchRow {
            prompt: r.n_prompt,
            generate: r.n_gen,
            depth: r.n_depth,
            tokens_per_s: r.samples_ts.iter().copied().reduce(f64::max).unwrap_or(r.avg_ts),
            samples: r.samples_ts,
            model_bytes: r.model_size,
        })
        .collect())
}

/// Where a model runs.
#[derive(Clone, Debug, PartialEq)]
pub enum Place {
    Gpu(String),
    Cpu,
}

#[derive(Clone, Debug)]
pub struct Engine {
    dir: PathBuf,
}

/// A child process that never outlives this one, however this one ends.
pub struct Guard {
    child: Child,
    leash: Option<Leash>,
}

impl Guard {
    pub fn spawn(mut c: Command) -> std::io::Result<Guard> {
        let mut g = Guard { child: c.spawn()?, leash: None };
        // Dropping `g` on failure ends the child.
        g.leash = Some(Leash::attach(&g.child)?);
        Ok(g)
    }

    pub fn stdout(&mut self) -> Option<std::process::ChildStdout> {
        self.child.stdout.take()
    }

    pub fn stderr(&mut self) -> Option<std::process::ChildStderr> {
        self.child.stderr.take()
    }

    pub fn ended(&mut self) -> std::io::Result<Option<std::process::ExitStatus>> {
        let s = self.child.try_wait()?;
        if s.is_some() {
            // Its process id may be used again by something else.
            if let Some(l) = self.leash.take() {
                l.discard();
            }
        }
        Ok(s)
    }

    /// Release what keeps the child alive, as the system does when this process ends.
    pub fn let_go(&mut self) {
        self.leash = None;
    }
}

impl Drop for Guard {
    fn drop(&mut self) {
        if let Some(l) = self.leash.take() {
            l.discard();
        }
        let _ = self.child.kill();
        let _ = self.child.wait();
    }
}

/// A watcher that ends the child when our end of its pipe closes.
#[cfg(unix)]
struct Leash(Child);

#[cfg(unix)]
impl Leash {
    fn attach(child: &Child) -> std::io::Result<Leash> {
        Command::new("/bin/sh")
            .args(["-c", &format!("read -r _; kill -9 {}", child.id())])
            .stdin(Stdio::piped())
            .stdout(Stdio::null())
            .stderr(Stdio::null())
            .spawn()
            .map(Leash)
    }

    fn discard(mut self) {
        let _ = self.0.kill();
        let _ = self.0.wait();
    }
}

/// A job that ends everything in it when our handle to it closes.
#[cfg(windows)]
struct Leash(#[allow(dead_code, reason = "held open, never read")] std::os::windows::io::OwnedHandle);

#[cfg(windows)]
impl Leash {
    fn attach(child: &Child) -> std::io::Result<Leash> {
        use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
        use windows_sys::Win32::System::JobObjects::*;
        let error = std::io::Error::last_os_error;
        // SAFETY: plain calls with valid handles; the job handle is owned straight away.
        unsafe {
            let raw = CreateJobObjectW(std::ptr::null(), std::ptr::null());
            if raw.is_null() {
                return Err(error());
            }
            let job = OwnedHandle::from_raw_handle(raw);
            let mut info: JOBOBJECT_EXTENDED_LIMIT_INFORMATION = std::mem::zeroed();
            info.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            let size = std::mem::size_of_val(&info) as u32;
            if SetInformationJobObject(raw, JobObjectExtendedLimitInformation, (&raw const info).cast(), size) == 0 || AssignProcessToJobObject(raw, child.as_raw_handle()) == 0 {
                return Err(error());
            }
            Ok(Leash(job))
        }
    }

    fn discard(self) {}
}

impl Engine {
    pub fn new(dir: impl Into<PathBuf>) -> Result<Self, String> {
        let e = Engine { dir: dir.into() };
        if e.tool("llama-bench").is_file() {
            Ok(e)
        } else {
            Err(format!("no engine in {}", e.dir.display()))
        }
    }

    /// `RUNIT_ENGINE_DIR`, else the folder shipped beside the program.
    pub fn locate() -> Result<Self, String> {
        if let Some(d) = std::env::var_os("RUNIT_ENGINE_DIR") {
            return Engine::new(d);
        }
        let exe = std::env::current_exe().map_err(|e| e.to_string())?;
        let exe = exe.canonicalize().unwrap_or(exe);
        let dir = exe.parent().ok_or("no program folder")?;
        [dir.join("engine"), dir.join("../Resources/engine"), dir.join("../engine")]
            .into_iter()
            .find_map(|d| Engine::new(d).ok())
            .ok_or_else(|| format!("no engine beside {}", exe.display()))
    }

    fn tool(&self, name: &str) -> PathBuf {
        self.dir.join(format!("{name}{}", std::env::consts::EXE_SUFFIX))
    }

    pub fn command(&self, name: &str) -> Command {
        let mut c = Command::new(self.tool(name));
        c.current_dir(&self.dir).stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::piped());
        #[cfg(windows)]
        {
            use std::os::windows::process::CommandExt;
            c.creation_flags(0x0800_0000); // no console window
        }
        c
    }

    pub fn devices(&self) -> Result<Vec<Device>, String> {
        let mut c = self.command("llama-bench");
        c.arg("--list-devices");
        Ok(parse_devices(&run(c, &AtomicBool::new(false), Duration::from_secs(180))?))
    }

    pub fn bench(&self, model: &Path, place: &Place, args: &[&str], stop: &AtomicBool) -> Result<Vec<BenchRow>, String> {
        self.bench_layers(model, place, 99, args, stop)
    }

    /// `layers` go to the graphics processor, the rest to the processor.
    pub fn bench_layers(&self, model: &Path, place: &Place, layers: u32, args: &[&str], stop: &AtomicBool) -> Result<Vec<BenchRow>, String> {
        let mut c = self.command("llama-bench");
        c.arg("-m").arg(model).args(["-o", "json"]).args(args);
        match place {
            Place::Gpu(id) => c.args(["-ngl", &layers.to_string(), "-dev", id]),
            Place::Cpu => c.args(["-ngl", "0", "-dev", "none"]),
        };
        parse_bench(&run(c, stop, Duration::from_secs(600))?)
    }
}

fn run(c: Command, stop: &AtomicBool, limit: Duration) -> Result<String, String> {
    let name = Path::new(c.get_program()).file_name().unwrap_or_default().to_string_lossy().into_owned();
    let mut g = Guard::spawn(c).map_err(|e| format!("could not start {name}: {e}"))?;
    let drain = |mut pipe: Box<dyn Read + Send>| {
        std::thread::spawn(move || {
            let mut v = Vec::new();
            let _ = pipe.read_to_end(&mut v);
            String::from_utf8_lossy(&v).into_owned()
        })
    };
    let out = drain(Box::new(g.stdout().ok_or("no output pipe")?));
    let log = drain(Box::new(g.stderr().ok_or("no log pipe")?));
    let start = Instant::now();
    let status = loop {
        if stop.load(Ordering::Relaxed) {
            return Err("stopped".into());
        }
        if start.elapsed() > limit {
            return Err(format!("{name} took too long"));
        }
        if let Some(s) = g.ended().map_err(|e| e.to_string())? {
            break s;
        }
        std::thread::sleep(Duration::from_millis(20));
    };
    let (out, log) = (out.join().unwrap_or_default(), log.join().unwrap_or_default());
    if status.success() {
        return Ok(out);
    }
    let why = log.lines().rev().find(|l| l.contains("error") || l.contains("fail") || l.contains("ASSERT")).unwrap_or("no reason given");
    Err(format!("{name} failed ({status}): {}", why.trim().chars().take(200).collect::<String>()))
}
