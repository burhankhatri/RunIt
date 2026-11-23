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
