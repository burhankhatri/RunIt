use std::ffi::{c_char, c_void, CStr, CString};
extern "C" fn ignore(_: *mut c_void, _: *const c_char) {}

#[test]
fn the_c_abi_answers_and_survives_bad_input() {
    std::env::set_var("RUNIT_DATA_DIR", std::env::temp_dir().join(format!("runit-ffi-{}", std::process::id())));
    unsafe {
        let app = runit::runit_open(std::ptr::null(), ignore, std::ptr::null_mut());
        assert!(!app.is_null());
        let ask = |m: &str, p: &str| {
            let (m, p) = (CString::new(m).unwrap(), CString::new(p).unwrap());
            let r = runit::runit_call(app, m.as_ptr(), p.as_ptr());
            let s = CStr::from_ptr(r).to_str().unwrap().to_string();
            runit::runit_free(r);
            serde_json::from_str::<serde_json::Value>(&s).unwrap()
        };
        assert!(ask("state", "{}")["ok"]["items"].is_array());
        assert!(ask("nope", "{}")["error"].is_string());
        assert!(ask("pause", "not json")["error"].is_string());
        let r = runit::runit_call(std::ptr::null(), std::ptr::null(), std::ptr::null());
        assert!(CStr::from_ptr(r).to_str().unwrap().contains("bad call"));
        runit::runit_free(r);
        runit::runit_free(std::ptr::null_mut());
        runit::runit_close(app);
    }
}
