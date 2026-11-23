//! Print creatures: `cargo run -p runit-core --example creatures -- "Phi-4" "Qwen3 4B"`
fn main() {
    for name in std::env::args().skip(1) {
        println!("{name}");
        let [a, b] = runit_core::creature::creature(&name);
        for y in 0..10 {
            let row = |f: &[u16; 10]| (0..10).map(|x| if f[y] >> x & 1 == 1 { "##" } else { "  " }).collect::<String>();
            println!("{}    {}", row(&a), row(&b));
        }
    }
}
