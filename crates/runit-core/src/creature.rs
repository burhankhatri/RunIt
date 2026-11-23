//! A small pixel creature for every name. The same name always gives the same one.

/// Ten rows of ten pixels, lowest bit on the left.
pub type Frame = [u16; 10];

/// Two frames of a walk.
pub fn creature(_name: &str) -> [Frame; 2] {
    [[0; 10]; 2]
}
