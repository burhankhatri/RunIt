use runit_core::conditions::*;

#[test]
fn battery_power_is_read_from_the_system_report() {
    assert!(on_battery("Now drawing from 'Battery Power'\n -InternalBattery-0 (id=1)\t81%; discharging;"));
    assert!(!on_battery("Now drawing from 'AC Power'\n -InternalBattery-0 (id=1)\t100%; charged;"));
    assert!(!on_battery(""));
}

#[test]
fn conditions_explain_themselves_only_when_something_is_off() {
    assert_eq!(Conditions { hot: false, on_battery: false }.note(), None);
    assert!(Conditions { hot: true, on_battery: false }.note().unwrap().contains("warm"));
    assert!(Conditions { hot: false, on_battery: true }.note().unwrap().contains("battery"));
    assert!(Conditions { hot: true, on_battery: true }.note().unwrap().contains("battery"));
    let _ = Conditions::now();
}
