//! What might be slowing this computer down right now.
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Conditions {
    /// The system is already holding speed back to shed heat.
    pub hot: bool,
    pub on_battery: bool,
}

/// Reads macOS `pmset -g batt`.
pub fn on_battery(report: &str) -> bool {
    report.lines().next().is_some_and(|l| l.contains("'Battery Power'"))
}

impl Conditions {
    #[cfg(target_os = "macos")]
    pub fn now() -> Conditions {
        let report = std::process::Command::new("/usr/bin/pmset").args(["-g", "batt"]).output().map(|o| String::from_utf8_lossy(&o.stdout).into_owned());
        Conditions { hot: thermal_pressure() > 0, on_battery: on_battery(&report.unwrap_or_default()) }
    }

    #[cfg(windows)]
    pub fn now() -> Conditions {
        use windows_sys::Win32::System::Power::{GetSystemPowerStatus, SYSTEM_POWER_STATUS};
        // Windows does not say when it is holding a processor back for heat.
        // SAFETY: the structure is plain data the call fills in.
        unsafe {
            let mut s: SYSTEM_POWER_STATUS = std::mem::zeroed();
            Conditions { hot: false, on_battery: GetSystemPowerStatus(&mut s) != 0 && s.ACLineStatus == 0 }
        }
    }

    #[cfg(not(any(windows, target_os = "macos")))]
    pub fn now() -> Conditions {
        Conditions::default()
    }

    pub fn note(&self) -> Option<&'static str> {
        match (self.hot, self.on_battery) {
            (true, true) => Some("This computer is warm and on battery, so it is running below its best."),
            (true, false) => Some("This computer is already warm, so it is running below its best."),
            (false, true) => Some("This computer is on battery, so it may run below its best."),
            (false, false) => None,
        }
    }
}

/// 0 is normal; 1 and up mean macOS is slowing things to cool down.
#[cfg(target_os = "macos")]
fn thermal_pressure() -> u64 {
    use std::ffi::{c_char, c_int};
    extern "C" {
        fn notify_register_check(name: *const c_char, token: *mut c_int) -> u32;
        fn notify_get_state(token: c_int, state: *mut u64) -> u32;
        fn notify_cancel(token: c_int) -> u32;
    }
    let (mut token, mut state) = (0, 0);
    // SAFETY: documented libnotify calls with valid pointers; the token is released.
    unsafe {
        if notify_register_check(c"com.apple.system.thermalpressurelevel".as_ptr(), &mut token) != 0 {
            return 0;
        }
        notify_get_state(token, &mut state);
        notify_cancel(token);
    }
    state
}
