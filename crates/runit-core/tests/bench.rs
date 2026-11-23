use runit_core::bench::*;
const GB: u64 = 1_000_000_000;

#[test]
fn sizes_fit_memory_and_disk() {
    assert_eq!(sizes(12 * GB, Some(50 * GB)).len(), 3);
    assert_eq!(sizes(12 * GB, None).len(), 3);
    // 2.2 GB free: the 2 GB model would fill the disk.
    assert_eq!(sizes(12 * GB, Some(2_200_000_000)).len(), 2);
    // A 2 GB card cannot hold the large one.
    assert_eq!(sizes(2 * GB, Some(50 * GB)).len(), 2);
    assert!(sizes(12 * GB, Some(300_000_000)).is_empty());
}

#[test]
fn sustained_share_compares_the_end_with_the_peak() {
    assert!((sustained(&[50.0, 49.0, 40.0, 32.0, 30.0, 31.0]) - 0.62).abs() < 0.001);
    assert_eq!(sustained(&[50.0, 50.0, 50.0]), 1.0);
    assert_eq!(sustained(&[40.0, 50.0, 51.0, 52.0]), 51.0 / 52.0);
    assert_eq!(sustained(&[]), 1.0);
}

#[test]
fn weak_graphics_give_way_to_a_quicker_processor() {
    use runit_core::predict::Sample;
    let s = |tok: f64| Sample { touched_bytes: 1e9, active_params: 1e9, layers: 20, decode_s: 1.0 / tok, prompt_s: 0.001 };
    assert!(processor_is_quicker(&s(6.0), &s(20.0)));
    assert!(!processor_is_quicker(&s(99.0), &s(77.0)));
    // A near tie stays on graphics, which reads prompts faster and leaves the processor free.
    assert!(!processor_is_quicker(&s(20.0), &s(21.0)));
}
