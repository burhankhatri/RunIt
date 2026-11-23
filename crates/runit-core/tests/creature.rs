use runit_core::creature::*;
use std::collections::HashSet;

const NAMES: [&str; 14] = ["Granite 4.2 8B", "Granite 4.2 3B", "Gemma 3n E4B IT", "Gemma 3 4B IT", "Phi-4", "Llama 3.2 3B Instruct", "gpt-oss-20b", "Qwen3 4B", "LFM2.5 8B A1B", "Apple M4", "", "a", "日本語", "AMD Ryzen 9"];
fn on(f: &Frame, x: i32, y: i32) -> bool { (0..10).contains(&x) && (0..10).contains(&y) && f[y as usize] >> x & 1 == 1 }

#[test]
fn creatures_are_symmetric_and_in_bounds() {
    for n in NAMES { for f in creature(n) { for (y, row) in f.iter().enumerate() {
        assert!(*row < 1 << 10);
        for x in 0..10 { assert_eq!(on(&f, x, y as i32), on(&f, 9 - x, y as i32), "{n} row {y}"); }
    } } }
}

#[test]
fn a_name_always_gives_the_same_creature_and_names_differ() {
    for n in NAMES { assert_eq!(creature(n), creature(n)); }
    assert_eq!(creature("Phi-4"), creature("PHI-4"), "capitals do not matter");
    assert!(NAMES.iter().map(|n| creature(n)).collect::<HashSet<_>>().len() >= 13);
    // Across many names nearly all are different.
    assert!((0..300).map(|i| creature(&format!("model {i}"))).collect::<HashSet<_>>().len() > 290);
}

#[test]
fn every_creature_is_one_piece_with_two_eyes() {
    for n in NAMES.iter().map(|s| s.to_string()).chain((0..200).map(|i| format!("model {i}"))) { for f in creature(&n) {
        for x in [3, 6] {
            assert!(!on(&f, x, 4), "{n}: eye");
            for (dx, dy) in [(-1, 0), (1, 0), (0, -1), (0, 1)] { assert!(on(&f, x + dx, 4 + dy), "{n}: an eye is open at the side"); }
        }
        let lit: HashSet<(i32, i32)> = (0..10).flat_map(|y| (0..10).map(move |x| (x, y))).filter(|(x, y)| on(&f, *x, *y)).collect();
        let (mut seen, mut todo) = (HashSet::new(), vec![(4, 5)]);
        // Diagonal neighbours touch, as they do to the eye.
        while let Some((x, y)) = todo.pop() { if lit.contains(&(x, y)) && seen.insert((x, y)) { for dx in -1..=1 { for dy in -1..=1 { todo.push((x + dx, y + dy)); } } } }
        assert_eq!(seen, lit, "{n}: in pieces");
        assert!((34..=76).contains(&lit.len()), "{n}: {} pixels", lit.len());
    } }
}

#[test]
fn the_two_frames_are_a_step() {
    for n in NAMES {
        let [a, b] = creature(n);
        assert_eq!(a[1..8], b[1..8], "{n}: only legs move");
        assert_ne!(a[8..], b[8..], "{n}");
    }
}
