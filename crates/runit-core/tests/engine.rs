use runit_core::engine::*;
use runit_core::hardware::Machine;

const GIB: u64 = 1 << 30;

#[test]
fn reads_metal_devices_and_ignores_devices_without_memory() {
    let d = parse_devices("ggml_metal_device_init: GPU name:   MTL0 (Apple M4)\nAvailable devices:\n  MTL0: Apple M4 (12124 MiB, 12123 MiB free)\n  BLAS: Accelerate (0 MiB, 0 MiB free)\n");
    assert_eq!(d, vec![Device { id: "MTL0".into(), name: "Apple M4".into(), total_mib: 12124, free_mib: 12123, integrated: true }]);
}

#[test]
fn prefers_a_discrete_card_over_a_bigger_shared_one() {
    let d = parse_devices("Available devices:\r\n  Vulkan0: Intel(R) UHD Graphics 770 (16200 MiB, 15390 MiB free)\r\n  Vulkan1: NVIDIA GeForce RTX 4070 Laptop GPU (7948 MiB, 7300 MiB free)\r\n");
    assert_eq!(d.len(), 2);
    assert!(d[0].integrated && !d[1].integrated);
    assert_eq!(best_device(&d).unwrap().id, "Vulkan1");
}

#[test]
fn knows_integrated_from_discrete_by_name() {
    for (name, integrated) in [("AMD Radeon(TM) Graphics", true), ("AMD Radeon 780M Graphics", true), ("AMD Radeon RX 7800 XT", false), ("AMD Radeon RX 7600M XT", false),
        ("Intel(R) Arc(TM) B580 Graphics", false), ("Intel(R) Arc(TM) A770 Graphics", false), ("Intel(R) Arc(TM) 140V GPU (16GB)", true), ("Intel(R) Iris(R) Xe Graphics", true), ("Qualcomm(R) Adreno(TM) X1-85 GPU", true), ("NVIDIA GeForce GTX 1660", false), ("Microsoft Basic Render Driver", true)] {
        let d = parse_devices(&format!("Available devices:\n  Vulkan0: {name} (8000 MiB, 7000 MiB free)\n"));
        assert_eq!(d[0].integrated, integrated, "{name}");
        assert_eq!(d[0].name, name);
    }
}

#[test]
fn no_devices_means_none() {
    assert!(parse_devices("Available devices:\n  (none)\n").is_empty());
    assert!(best_device(&[]).is_none());
}

#[test]
fn reads_bench_rows_by_best_sample_around_log_noise() {
    let out = r#"noise
[
 {"n_prompt":512,"n_gen":0,"n_depth":0,"avg_ts":400.0,"samples_ts":[500.0,390.0,400.0],"model_size":1060047360},
 {"n_prompt":0,"n_gen":64,"n_depth":4096,"avg_ts":63.0,"samples_ts":[],"model_size":1060047360}
]"#;
    let r = parse_bench(out).unwrap();
    // Other work can only slow a run down, so the best sample is the machine's own speed.
    assert_eq!(r[0], BenchRow { prompt: 512, generate: 0, depth: 0, tokens_per_s: 500.0, samples: vec![500.0, 390.0, 400.0], model_bytes: 1060047360 });
    assert_eq!((r[1].depth, r[1].tokens_per_s), (4096, 63.0));
    assert!(parse_bench("").is_err());
    assert!(parse_bench("[]").is_err());
}

fn machine(ram: u64, gpu: Option<Device>) -> Machine {
    Machine { os: "t".into(), cpu: "t".into(), cores: 8, threads: 8, performance_cores: 4, ram_bytes: ram, ram_free_bytes: ram / 2, gpu }
}
fn dev(mib: u64, free: u64, integrated: bool) -> Device { Device { id: "x".into(), name: "x".into(), total_mib: mib, free_mib: free, integrated } }

#[test]
fn unified_memory_leaves_room_for_the_system() {
    // 16 GiB: 25% is 4 GiB, leaving 12; the GPU budget of 12124 MiB is smaller.
    assert_eq!(machine(16 * GIB, Some(dev(12124, 12123, true))).capacity(), 12124 << 20);
    // 8 GiB machine keeps 4 GiB for the system.
    assert_eq!(machine(8 * GIB, Some(dev(5461, 5461, true))).capacity(), 4 * GIB);
}

#[test]
fn discrete_capacity_is_free_video_memory_and_no_gpu_means_ram() {
    assert_eq!(machine(32 * GIB, Some(dev(8192, 7300, false))).capacity(), 7300 << 20);
    assert_eq!(machine(32 * GIB, None).capacity(), 24 * GIB);
    assert_eq!(machine(32 * GIB, Some(dev(8192, 7300, false))).ram_capacity(), 24 * GIB);
}

fn alive(pid: u32) -> bool {
    #[cfg(unix)]
    { std::process::Command::new("kill").args(["-0", &pid.to_string()]).stderr(std::process::Stdio::null()).status().unwrap().success() }
    #[cfg(windows)]
    { String::from_utf8_lossy(&std::process::Command::new("tasklist").args(["/FI", &format!("PID eq {pid}")]).output().unwrap().stdout).contains(&pid.to_string()) }
}
/// A long-running child that prints its own process id first, so the test can watch it.
fn sleeper() -> (Guard, u32) {
    use std::io::{BufRead, BufReader};
    #[cfg(unix)]
    let mut c = { let mut c = std::process::Command::new("/bin/sh"); c.args(["-c", "echo $$; exec sleep 60"]); c };
    #[cfg(windows)]
    let mut c = { let mut c = std::process::Command::new("powershell"); c.args(["-NoProfile", "-Command", "Write-Output $PID; Start-Sleep 60"]); c };
    c.stdout(std::process::Stdio::piped()).stderr(std::process::Stdio::null());
    let mut g = Guard::spawn(c).unwrap();
    let mut line = String::new();
    BufReader::new(g.stdout().unwrap()).read_line(&mut line).unwrap();
    (g, line.trim().parse().unwrap())
}
fn gone_soon(pid: u32) -> bool {
    (0..50).any(|_| { std::thread::sleep(std::time::Duration::from_millis(100)); !alive(pid) })
}

#[test]
fn dropping_a_guard_ends_its_child() {
    let (g, pid) = sleeper();
    assert!(alive(pid));
    drop(g);
    assert!(gone_soon(pid));
}

#[test]
fn a_child_does_not_outlive_a_parent_that_vanishes() {
    let (mut g, pid) = sleeper();
    // What the system does to our handles when this process is killed outright.
    g.let_go();
    // Asking how it ended also clears the zombie a killed child leaves with a living parent.
    let ended = (0..50).any(|_| { std::thread::sleep(std::time::Duration::from_millis(100)); g.ended().unwrap().is_some() });
    assert!(ended && !alive(pid), "child {pid} was left behind");
}

#[test]
fn a_guard_reports_how_its_child_ended() {
    #[cfg(unix)]
    let c = { let mut c = std::process::Command::new("/bin/sh"); c.args(["-c", "exit 3"]); c };
    #[cfg(windows)]
    let c = { let mut c = std::process::Command::new("cmd"); c.args(["/C", "exit 3"]); c };
    let mut g = Guard::spawn(c).unwrap();
    let code = (0..100).find_map(|_| { std::thread::sleep(std::time::Duration::from_millis(50)); g.ended().unwrap() }).unwrap();
    assert_eq!(code.code(), Some(3));
}
