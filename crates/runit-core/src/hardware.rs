//! What this computer is.
use crate::engine::{best_device, Device, Engine};
use serde::{Deserialize, Serialize};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Machine {
    pub os: String,
    pub cpu: String,
    pub cores: u32,
    pub threads: u32,
    /// Fast cores, when the processor mixes two kinds. Otherwise all cores.
    pub performance_cores: u32,
    pub ram_bytes: u64,
    pub ram_free_bytes: u64,
    pub gpu: Option<Device>,
}

const GIB: u64 = 1 << 30;

impl Machine {
    pub fn detect(engine: &Engine) -> Machine {
        let mut s = sysinfo::System::new();
        s.refresh_memory();
        s.refresh_cpu_all();
        let cores = sysinfo::System::physical_core_count().unwrap_or(1) as u32;
        // A machine without a working GPU driver is still a machine.
        let devices = engine.devices().unwrap_or_default();
        Machine {
            os: sysinfo::System::long_os_version().unwrap_or_else(|| std::env::consts::OS.into()),
            cpu: s.cpus().first().map(|c| tidy_cpu(c.brand())).unwrap_or_default(),
            cores,
            threads: s.cpus().len() as u32,
            performance_cores: performance_cores().unwrap_or(cores),
            ram_bytes: s.total_memory(),
            ram_free_bytes: s.available_memory(),
            gpu: best_device(&devices).cloned(),
        }
    }

    /// Bytes usable in system memory, leaving room for everything else that is open.
    pub fn ram_capacity(&self) -> u64 {
        self.ram_bytes.saturating_sub((self.ram_bytes / 4).max(4 * GIB))
    }

    /// Bytes a model may occupy while running fully accelerated.
    pub fn capacity(&self) -> u64 {
        match &self.gpu {
            Some(g) if g.integrated => (g.total_mib << 20).min(self.ram_capacity()),
            Some(g) => g.free_mib << 20,
            None => self.ram_capacity(),
        }
    }

    /// Changes when results measured on this machine no longer apply.
    pub fn fingerprint(&self) -> String {
        let gpu = self.gpu.as_ref().map_or("none", |g| g.name.as_str());
        format!("{}|{}|{}|{gpu}", self.cpu, self.cores, self.ram_bytes >> 30)
    }
}

fn tidy_cpu(brand: &str) -> String {
    let b = brand.replace("(R)", "").replace("(TM)", "").replace("(r)", "").replace("(tm)", "");
    let b = b.split(" @ ").next().unwrap_or(&b).replace(" CPU", "").replace(" Processor", "");
    b.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn performance_cores() -> Option<u32> {
    if !cfg!(target_os = "macos") {
        return None;
    }
    let out = std::process::Command::new("/usr/sbin/sysctl").args(["-n", "hw.perflevel0.physicalcpu"]).output().ok()?;
    String::from_utf8(out.stdout).ok()?.trim().parse().ok()
}

pub fn disk_free(path: &Path) -> Option<u64> {
    let mut p = path;
    while !p.exists() {
        p = p.parent()?;
    }
    fs4::available_space(p).ok()
}
