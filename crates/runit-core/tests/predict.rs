use runit_core::engine::Device;
use runit_core::gguf::Shape;
use runit_core::hardware::Machine;
use runit_core::predict::*;

const GIB: u64 = 1 << 30;

fn shape() -> Shape {
    Shape { arch: "llama".into(), layers: 28, embedding: 3072, vocab: 128256, heads: 24, kv_heads: vec![8; 28], key_length: 128, value_length: 128,
        feed_forward: vec![8192; 28], experts: 0, experts_used: 0, expert_feed_forward: 0, dense_lead: 0, per_layer_input: 0, context: 131072,
        sliding_window: 0, file_type: Some(15), chat_template: true, tied_embeddings: Some(true) }
}
fn machine(ram_gib: u64, gpu: Option<(u64, bool)>) -> Machine {
    Machine { os: "t".into(), cpu: "t".into(), cores: 8, threads: 8, performance_cores: 4, ram_bytes: ram_gib * GIB, ram_free_bytes: ram_gib * GIB,
        gpu: gpu.map(|(mib, integrated)| Device { id: "x".into(), name: "x".into(), total_mib: mib, free_mib: mib, integrated }) }
}
fn close(a: f64, b: f64, pct: f64) { assert!((a - b).abs() <= b.abs() * pct / 100.0, "{a} vs {b}"); }

#[test]
fn tied_models_touch_every_byte_untied_skip_the_embedding_table() {
    let tied = load_from_shape(&shape(), 2_019_377_600);
    close(tied.touched_bytes, 2_019_377_600.0, 0.5);
    let untied = load_from_shape(&Shape { tied_embeddings: Some(false), ..shape() }, 2_130_000_000);
    close(untied.touched_bytes / 2_130_000_000.0, 1.0 - 0.1094, 1.0);
    // KV: 28 layers * 8 heads * 256 * 2 bytes.
    assert_eq!(tied.kv_bytes_per_token, 28.0 * 8.0 * 256.0 * 2.0);
    assert!(tied.from_header);
}

#[test]
fn an_unknown_tie_is_settled_by_which_explanation_fits_the_file() {
    let unknown = Shape { tied_embeddings: None, ..shape() };
    close(load_from_shape(&unknown, 2_019_377_600).touched_bytes, 2_019_377_600.0, 0.5); // Llama 3.2 3B is tied
    let qwen = Shape { embedding: 1536, vocab: 151936, heads: 12, kv_heads: vec![2; 28], feed_forward: vec![8960; 28], ..unknown }; // ships output.weight
    assert!(load_from_shape(&qwen, 1_117_000_000).touched_bytes < 1_010_000_000.0);
}

#[test]
fn a_mixture_of_experts_reads_only_the_experts_it_uses() {
    let moe = Shape { experts: 128, experts_used: 8, expert_feed_forward: 768, layers: 48, kv_heads: vec![4; 48], feed_forward: vec![6144; 48], embedding: 2048, heads: 32, key_length: 128, value_length: 128, vocab: 151936, tied_embeddings: Some(false), ..shape() };
    let l = load_from_shape(&moe, 18_600_000_000); // Qwen3-30B-A3B: about a tenth is active
    assert!(l.touched_bytes > 1.6e9 && l.touched_bytes < 2.6e9, "{}", l.touched_bytes);
    assert!(l.active_params > 2.5e9 && l.active_params < 4e9, "{}", l.active_params);
}

#[test]
fn per_layer_embeddings_are_not_read_in_full() {
    let g3n = Shape { layers: 35, embedding: 2048, vocab: 262144, heads: 8, kv_heads: vec![2; 35], key_length: 256, value_length: 256, feed_forward: vec![16384; 35], per_layer_input: 256, ..shape() };
    let l = load_from_shape(&g3n, 4_240_000_000);
    assert!(l.touched_bytes < 3_300_000_000.0, "{}", l.touched_bytes);
}

#[test]
fn fit_recovers_known_constants() {
    let s = |bytes: f64, params: f64, layers: u32| Sample { touched_bytes: bytes, active_params: params, layers, decode_s: bytes / 100e9 + layers as f64 * 50e-6, prompt_s: params * 0.7e-12 + layers as f64 * 2e-6 };
    let f = fit(&[s(0.16e9, 0.27e9, 16), s(0.93e9, 1.5e9, 28), s(1.9e9, 3.2e9, 28)], None).unwrap();
    close(1.0 / f.decode_s_per_byte, 100e9, 0.1);
    close(f.decode_s_per_layer, 50e-6, 0.1);
    close(f.prompt_s_per_param, 0.7e-12, 0.1);
    // Without a deep sample, KV is read at the weight bandwidth.
    close(f.kv_s_per_byte, f.decode_s_per_byte, 0.01);
}

#[test]
fn fit_never_goes_negative_and_needs_two_points() {
    let s = |b: f64, t: f64, l: u32| Sample { touched_bytes: b, active_params: b, layers: l, decode_s: t, prompt_s: t / 10.0 };
    let f = fit(&[s(1e9, 0.0100, 20), s(2e9, 0.0205, 20)], None).unwrap();
    assert!(f.decode_s_per_layer >= 0.0 && f.decode_s_per_byte > 0.0 && f.prompt_s_per_layer >= 0.0);
    assert!(fit(&[s(1e9, 0.01, 20)], None).is_none());
    assert!(fit(&[s(1e9, f64::NAN, 20), s(1e9, 0.0, 20)], None).is_none());
}

#[test]
fn a_deep_sample_sets_the_cost_of_reading_context() {
    let s = |b: f64, l: u32| Sample { touched_bytes: b, active_params: b, layers: l, decode_s: b / 100e9, prompt_s: b / 1e12 };
    // 4e8 bytes of KV cost 2 ms more than the shallow run of the same model.
    let f = fit(&[s(1e9, 20), s(2e9, 28)], Some((s(1e9, 20), 4e8, 0.012))).unwrap();
    close(f.kv_s_per_byte, 0.002 / 4e8, 1.0);
}

fn cal() -> Calibration {
    let f = |bw: f64| Fit { decode_s_per_byte: 1.0 / bw, decode_s_per_layer: 0.0, prompt_s_per_param: 1e-12, prompt_s_per_layer: 0.0, kv_s_per_byte: 0.0 };
    Calibration { gpu: Some(f(100e9)), cpu: f(20e9) }
}

#[test]
fn speed_follows_bytes_and_tiers_follow_memory() {
    let m = machine(16, Some((12124, true)));
    let small = predict(&load_from_size(2_000_000_000), &m, &cal());
    assert_eq!(small.tier, Tier::Comfortable);
    close(small.tokens_per_s, 50.0, 3.0);
    assert!(small.rough && small.tokens_per_s_low < small.tokens_per_s && small.tokens_per_s < small.tokens_per_s_high);
    assert_eq!(predict(&load_from_size(9_800_000_000), &m, &cal()).tier, Tier::Tight);
    assert_eq!(predict(&load_from_size(40_000_000_000), &m, &cal()).tier, Tier::TooBig);
    assert_eq!(predict(&load_from_size(40_000_000_000), &m, &cal()).tokens_per_s, 0.0);
}

#[test]
fn a_model_over_video_memory_is_split_and_slower() {
    let m = machine(32, Some((8192, false)));
    let p = predict(&load_from_size(15_000_000_000), &m, &cal());
    assert_eq!(p.tier, Tier::Partial);
    assert!(p.gpu_share > 0.4 && p.gpu_share < 0.6, "{}", p.gpu_share);
    // Half at 100 GB/s and half at 20 GB/s: about 2.2 tokens/s, far below 6.7 on the GPU alone.
    assert!(p.tokens_per_s > 1.6 && p.tokens_per_s < 2.8, "{}", p.tokens_per_s);
}

#[test]
fn without_a_gpu_the_cpu_fit_is_used() {
    let c = Calibration { gpu: None, ..cal() };
    let p = predict(&load_from_size(2_000_000_000), &machine(16, None), &c);
    close(p.tokens_per_s, 10.0, 3.0);
    assert_eq!((p.tier, p.gpu_share), (Tier::Comfortable, 0.0));
}

#[test]
fn a_long_conversation_is_never_faster() {
    let mut c = cal();
    c.gpu.as_mut().unwrap().kv_s_per_byte = 1.0 / 50e9;
    let p = predict(&load_from_shape(&shape(), 2_019_377_600), &machine(16, Some((12124, true))), &c);
    assert!(p.tokens_per_s_deep < p.tokens_per_s);
    assert!(!p.rough);
    assert_eq!(p.context, 8192);
}
