//! C ABI for the native apps. Strings are UTF-8; every returned string goes back to `runit_free`.
use runit_core::{app::App, store, Emit};
use serde_json::{json, Value};
use std::ffi::{c_char, c_void, CStr, CString};
use std::panic::{catch_unwind, AssertUnwindSafe};
use std::path::PathBuf;
use std::sync::Arc;

pub type Callback = extern "C" fn(context: *mut c_void, event: *const c_char);

struct Context(*mut c_void);
// The host promises its context may be used from any thread.
unsafe impl Send for Context {}
unsafe impl Sync for Context {}

fn out(v: Value) -> *mut c_char {
    CString::new(v.to_string()).map_or(std::ptr::null_mut(), CString::into_raw)
}

unsafe fn text<'a>(p: *const c_char) -> Option<&'a str> {
    if p.is_null() {
        return None;
    }
    CStr::from_ptr(p).to_str().ok()
}

/// Start RunIt. `engine_dir` may be null to look beside the program. Null means it could not start.
///
/// # Safety
/// `engine_dir` is null or a valid string. `context` must outlive the returned handle.
#[no_mangle]
pub unsafe extern "C" fn runit_open(engine_dir: *const c_char, callback: Callback, context: *mut c_void) -> *mut App {
    let engine = text(engine_dir).map(PathBuf::from);
    let context = Context(context);
    let emit: Emit = Arc::new(move |v| {
        let context = &context;
        if let Ok(s) = CString::new(v.to_string()) {
            callback(context.0, s.as_ptr());
        }
    });
    match catch_unwind(AssertUnwindSafe(|| App::open(store::data_dir(), engine, emit))) {
        Ok(Ok(app)) => Box::into_raw(Box::new(app)),
        _ => std::ptr::null_mut(),
    }
}

/// Call by name with JSON parameters. Answers `{"ok": value}` or `{"error": "why"}`. May block on
/// the network, so call it off the main thread.
///
/// # Safety
/// `app` came from `runit_open`; the strings are valid.
#[no_mangle]
pub unsafe extern "C" fn runit_call(app: *const App, method: *const c_char, params: *const c_char) -> *mut c_char {
    let (Some(app), Some(method)) = (app.as_ref(), text(method)) else { return out(json!({"error": "bad call"})) };
    let params: Value = text(params).and_then(|p| serde_json::from_str(p).ok()).unwrap_or(Value::Null);
    out(match catch_unwind(AssertUnwindSafe(|| app.call(method, &params))) {
        Ok(Ok(v)) => json!({"ok": v}),
        Ok(Err(e)) => json!({"error": e}),
        Err(_) => json!({"error": "something went wrong inside RunIt"}),
    })
}

/// # Safety
/// `s` came from this library and is freed once.
#[no_mangle]
pub unsafe extern "C" fn runit_free(s: *mut c_char) {
    if !s.is_null() {
        drop(CString::from_raw(s));
    }
}

/// # Safety
/// `app` came from `runit_open` and is closed once, with no call in progress.
#[no_mangle]
pub unsafe extern "C" fn runit_close(app: *mut App) {
    if !app.is_null() {
        drop(Box::from_raw(app));
    }
}
