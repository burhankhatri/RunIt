use runit_core::{catalog::*, engine::Device, hardware::Machine, predict::*, quality::Ratings, recommend::*};

const GB: u64 = 1_000_000_000;
fn v(label: &str, bits: f64, size: u64) -> Variant { Variant { label: label.into(), bits, size, files: vec![FileRow { path: format!("m-{label}.gguf"), size, sha256: format!("{label}{size}"), url: "https://huggingbay.xyz/api/downloads/x".into() }] } }
fn e(repo: &str, vs: Vec<Variant>) -> Entry { Entry { id: repo.to_lowercase(), repo: repo.into(), kind: "llm".into(), license: "apache-2.0".into(), downloads: 10, variants: vs } }
fn m() -> Machine { Machine { os: "t".into(), cpu: "t".into(), cores: 8, threads: 8, performance_cores: 4, ram_bytes: 16 << 30, ram_free_bytes: 8 << 30, gpu: Some(Device { id: "MTL0".into(), name: "M".into(), total_mib: 12124, free_mib: 12124, integrated: true }) } }
fn c() -> Calibration { let f = |bw: f64| Fit { decode_s_per_byte: 1.0 / bw, decode_s_per_layer: 0.0, prompt_s_per_param: 1e-12, prompt_s_per_layer: 0.0, kv_s_per_byte: 0.0 }; Calibration { gpu: Some(f(100e9)), cpu: f(30e9), sustained: 1.0 } }
fn run(es: &[Entry]) -> Shortlist { shortlist(es, &Ratings::bundled(), &m(), &c(), &|v| vec![None; v.len()]) }
fn role<'a>(s: &'a Shortlist, r: &str) -> Option<&'a str> { s.picks.iter().find(|p| p.role == r).map(|p| p.choice.repo.as_str()) }

fn catalog() -> Vec<Entry> {
    vec![
        e("unsloth/gemma-4-26B-A4B-it-GGUF", vec![v("Q4_K_M", 4.9, 168 * GB / 10)]),     // 1434, too big here
        e("ibm-granite/granite-4.2-8b-GGUF", vec![v("Q4_K_M", 4.9, 53 * GB / 10), v("Q8_0", 8.5, 94 * GB / 10)]), // 1319, 19 tok/s
        e("ibm-granite/granite-4.2-3b-GGUF", vec![v("Q2_K", 3.2, 146 * GB / 100), v("Q4_K_M", 4.9, 22 * GB / 10)]), // 1293, 45 tok/s
        e("unsloth/Llama-3.2-3B-Instruct-GGUF", vec![v("Q4_K_M", 4.9, 2 * GB)]),          // 1110, 50 tok/s
        e("bartowski/gemma-3-12b-it-GGUF", vec![v("Q4_K_M", 4.9, 104 * GB / 10)]),          // 1334, 9.6 tok/s, tight
        e("somebody/Qwen3.8-9B-Distill-GGUF", vec![v("Q4_K_M", 4.9, 5 * GB)]),             // unrated
    ]
}

#[test]
fn each_role_goes_to_the_best_model_that_meets_its_bar() {
    let s = run(&catalog());
    assert_eq!(role(&s, "best"), Some("ibm-granite/granite-4.2-8b-GGUF"));
    assert_eq!(role(&s, "quick"), Some("ibm-granite/granite-4.2-3b-GGUF"));
    assert_eq!(role(&s, "smart"), Some("bartowski/gemma-3-12b-it-GGUF"));
    for p in &s.picks { assert!(!p.why.is_empty() && p.choice.prediction.tokens_per_s > 0.0); }
}

#[test]
fn nothing_unrated_too_big_or_heavily_squeezed_is_listed() {
    let s = run(&catalog());
    assert!(s.ranked.iter().all(|c| !c.repo.contains("Distill") && !c.repo.contains("gemma-4-26B")));
    assert!(s.ranked.iter().all(|c| c.variant.bits >= 4.0 && c.prediction.tier <= Tier::Tight));
}

#[test]
fn one_model_appears_once_preferring_its_own_publisher() {
    let mut es = catalog();
    es.push(e("unsloth/granite-4.2-8b-GGUF", vec![v("Q4_K_M", 4.9, 53 * GB / 10)]));
    es.push(e("nobody/granite-4.2-8b-GGUF", vec![v("Q4_K_M", 4.9, 53 * GB / 10)]));
    let s = run(&es);
    let same: Vec<_> = s.ranked.iter().filter(|c| c.name == "Granite 4.2 8B").map(|c| c.repo.as_str()).collect();
    assert_eq!(same, ["ibm-granite/granite-4.2-8b-GGUF"]);
}

#[test]
fn unknown_uploaders_are_not_recommended() {
    assert!(run(&[e("nobody/granite-4.2-8b-GGUF", vec![v("Q4_K_M", 4.9, 5 * GB)])]).ranked.is_empty());
}

#[test]
fn a_finer_quant_is_chosen_only_while_it_stays_quick() {
    // 3B at Q8 is 4.2 GB: 24 tok/s, under the 40 that would justify it. At 2.3 GB it is 43.
    let slow = run(&[e("ibm-granite/granite-4.2-3b-GGUF", vec![v("Q4_K_M", 4.9, 22 * GB / 10), v("Q8_0", 8.5, 42 * GB / 10)])]);
    assert_eq!(slow.ranked[0].variant.label, "Q4_K_M");
    let quick = run(&[e("ibm-granite/granite-4.2-3b-GGUF", vec![v("Q4_K_M", 4.9, 2 * GB), v("Q6_K", 6.6, 23 * GB / 10)])]);
    assert_eq!(quick.ranked[0].variant.label, "Q6_K");
}

#[test]
fn a_model_that_is_several_bests_is_shown_once() {
    let s = run(&[e("ibm-granite/granite-4.2-3b-GGUF", vec![v("Q4_K_M", 4.9, 22 * GB / 10)])]);
    assert_eq!(s.picks.len(), 1);
    assert_eq!(s.picks[0].role, "best");
    assert_eq!(s.picks[0].also, ["quick", "code", "smart"]);
}

#[test]
fn header_loads_replace_the_size_estimate() {
    let es = [e("ibm-granite/granite-4.2-8b-GGUF", vec![v("Q4_K_M", 4.9, 53 * GB / 10)])];
    let s = shortlist(&es, &Ratings::bundled(), &m(), &c(), &|vs| vs.iter().map(|v| Some(Load { from_header: true, touched_bytes: 1e9, ..load_from_size(v.size) })).collect());
    assert!(!s.ranked[0].prediction.rough && s.ranked[0].prediction.tokens_per_s > 90.0);
}

#[test]
fn a_file_far_too_small_to_be_the_named_model_is_not_that_model() {
    // ggml-org/gpt-oss-120b-GGUF also ships a 0.8 GB helper for it.
    let s = run(&[e("ggml-org/gpt-oss-120b-GGUF", vec![v("Q8_0", 8.5, 8 * GB / 10)])]);
    assert!(s.ranked.is_empty());
    // "E4B" and "8x7B" style names are left alone; "26B A4B" is judged on 26B.
    assert_eq!(run(&[e("ggml-org/gemma-3n-E4B-it-GGUF", vec![v("Q4_K_M", 4.9, 42 * GB / 10)])]).ranked.len(), 1);
}

#[test]
fn popular_models_without_a_rating_are_kept_apart() {
    let s = run(&[
        Entry { downloads: 500, ..e("LiquidAI/LFM2.5-8B-A1B-GGUF", vec![v("Q4_K_M", 4.9, 5 * GB)]) },
        Entry { downloads: 900, ..e("somebody/Qwen3.8-9B-Distill-GGUF", vec![v("Q4_K_M", 4.9, 5 * GB)]) },
        Entry { downloads: 100, ..e("Qwen/Qwen3-4B-GGUF", vec![v("Q5_K_M", 5.7, 3 * GB)]) },
        Entry { downloads: 800, ..e("Qwen/Qwen3-30B-A3B-Coder-GGUF", vec![v("Q4_K_M", 4.9, 18 * GB)]) },
    ]);
    assert!(s.ranked.is_empty() && s.picks.is_empty());
    assert_eq!(s.unrated.iter().map(|c| c.repo.as_str()).collect::<Vec<_>>(), ["LiquidAI/LFM2.5-8B-A1B-GGUF", "Qwen/Qwen3-4B-GGUF"]);
    assert!(s.unrated.iter().all(|c| c.rating.is_none()));
}

#[test]
fn a_slow_best_is_not_called_smooth() {
    let slow = { let f = Fit { decode_s_per_byte: 1.0 / 20e9, decode_s_per_layer: 0.0, prompt_s_per_param: 1e-12, prompt_s_per_layer: 0.0, kv_s_per_byte: 0.0 }; Calibration { gpu: None, cpu: f, sustained: 1.0 } };
    let s = shortlist(&[e("ibm-granite/granite-4.2-3b-GGUF", vec![v("Q4_K_M", 4.9, 22 * GB / 10)])], &Ratings::bundled(), &m(), &slow, &|v| vec![None; v.len()]);
    assert_eq!(s.picks[0].role, "best");
    assert!(s.picks[0].choice.prediction.tokens_per_s < 15.0);
    assert!(!s.picks[0].why.contains("smooth"), "{}", s.picks[0].why);
}

#[test]
fn a_much_better_model_above_reading_speed_beats_a_quicker_weak_one() {
    let f = Fit { decode_s_per_byte: 1.0 / 25e9, decode_s_per_layer: 0.0, prompt_s_per_param: 1e-12, prompt_s_per_layer: 0.0, kv_s_per_byte: 0.0 };
    let slow = Calibration { gpu: None, cpu: f, sustained: 1.0 };
    let s = shortlist(&[
        e("ibm-granite/granite-4.2-3b-GGUF", vec![v("Q4_K_M", 4.9, 22 * GB / 10)]),          // 1293 at 11 tok/s
        e("bartowski/granite-3.1-2b-instruct-GGUF", vec![v("Q4_K_M", 4.9, 15 * GB / 10)]),   // 1127 at 16 tok/s
    ], &Ratings::bundled(), &m(), &slow, &|v| vec![None; v.len()]);
    assert_eq!(role(&s, "best"), Some("ibm-granite/granite-4.2-3b-GGUF"));
}
