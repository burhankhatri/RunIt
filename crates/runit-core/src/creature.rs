//! A small pixel creature for every name. The same name always gives the same one.

/// Ten rows of ten pixels, lowest bit on the left.
pub type Frame = [u16; 10];

struct Dice(u64);

impl Dice {
    fn roll(&mut self, below: u64) -> u64 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 20) % below
    }
    fn chance(&mut self, percent: u64) -> bool {
        self.roll(100) < percent
    }
}

/// Two frames of a walk.
pub fn creature(name: &str) -> [Frame; 2] {
    // FNV-1a, then one step so short names spread out.
    let seed = name.to_lowercase().bytes().fold(0xcbf2_9ce4_8422_2325u64, |h, b| (h ^ b as u64).wrapping_mul(0x100_0000_01b3));
    let mut d = Dice(seed | 1);
    d.roll(2);

    // Left half only: column 0 is the outside, column 4 touches the middle. Each row is a run
    // that reaches the middle, so the body is always one piece.
    let mut half = [[false; 5]; 10];
    let fill = |row: &mut [bool; 5], from: usize| row[from..].fill(true);
    fill(&mut half[1], 2 + d.roll(3) as usize);
    fill(&mut half[2], 1 + d.roll(3) as usize);
    // Face: rows 3 to 5 always surround the eye at column 3.
    for y in 3..=5 {
        let from = d.roll(3) as usize;
        fill(&mut half[y], from.min(2));
    }
    half[4][3] = false;
    fill(&mut half[6], d.roll(4) as usize);
    fill(&mut half[7], 1 + d.roll(3) as usize);
    // A mouth or a belly mark, never cutting the body in two.
    if d.chance(45) {
        half[6][4] = false;
    } else if d.chance(50) && half[6][2] {
        half[6][3] = false;
    }
    // Antennae or ears.
    let top = (0..5).find(|x| half[1][*x]).unwrap_or(4);
    match d.roll(3) {
        0 => half[0][top] = true,
        1 => half[0][top.saturating_sub(1)] = true,
        _ => {
            half[0][top] = true;
            half[0][4] = d.chance(50);
        }
    }
    // Arms.
    if d.chance(60) {
        let y = 4 + d.roll(2) as usize;
        half[y][..2].fill(true);
        if d.chance(50) {
            half[y - 1][0] = true;
        }
    }
    // Legs under the body. The feet move on the second frame.
    let hip = (0..5).find(|x| half[7][*x]).unwrap_or(3);
    let leg = (hip + d.roll(2) as usize).min(3);
    let other = if leg >= 1 && d.chance(70) { leg - 1 } else { leg + 1 };
    let mut step = half;
    for (frame, foot) in [(&mut half, leg), (&mut step, other)] {
        frame[8][leg] = true;
        frame[9][foot] = true;
    }

    let whole = |h: &[[bool; 5]; 10]| {
        let mut f = [0u16; 10];
        for (row, cells) in f.iter_mut().zip(h) {
            for (x, on) in cells.iter().enumerate() {
                if *on {
                    *row |= 1 << x | 1 << (9 - x);
                }
            }
        }
        f
    };
    let (a, b) = (whole(&half), whole(&step));
    [a, b]
}
