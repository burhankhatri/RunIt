use runit_core::catalog::*;

fn f(path: &str, size: u64) -> FileRow { FileRow { path: path.into(), size, sha256: "a".repeat(64), url: format!("https://x/{path}") } }

#[test]
fn distils_the_real_snapshot_to_hosted_gguf_models() {
    let text = include_str!("fixtures/snapshot.jsonl");
    let e = distil(text.as_bytes()).unwrap();
    let llama = e.iter().find(|e| e.repo == "unsloth/Llama-3.2-3B-Instruct-GGUF").unwrap();
    assert_eq!(llama.id, "hf-model-unsloth-llama-3-2-3b-instruct-gguf");
    assert_eq!((llama.variants.len(), llama.variants[0].label.as_str(), llama.variants[0].size), (1, "Q4_K_M", 2_019_377_600));
    assert_eq!(llama.variants[0].files[0].sha256.len(), 64);
    assert!(llama.variants[0].files[0].url.starts_with("https://huggingbay.xyz/api/downloads/"));
    assert!(e.iter().all(|e| !e.repo.starts_with("arxiv/") && !e.variants.is_empty()));
    assert!(distil("not json\n".as_bytes()).is_err());
}

#[test]
fn shards_become_one_choice_and_side_files_are_left_out() {
    let v = variants(&[f("BF16/m-BF16-00001-of-00002.gguf", 10), f("BF16/m-BF16-00002-of-00002.gguf", 5), f("m-Q4_K_M.gguf", 4), f("mmproj-F16.gguf", 1),
        f("m.imatrix.gguf", 1), f("m-UD-IQ2_XXS.gguf", 2), f("README.md", 1), f("m-IQ3_M.gguf", 3), f("config.json", 1)]);
    let l: Vec<_> = v.iter().map(|v| (v.label.as_str(), v.size, v.files.len())).collect();
    assert_eq!(l, vec![("IQ2_XXS", 2, 1), ("IQ3_M", 3, 1), ("Q4_K_M", 4, 1), ("BF16", 15, 2)]);
    assert!(v[0].bits < v[1].bits && v[1].bits < 4.0 && v[2].bits >= 4.0);
}

#[test]
fn a_missing_shard_is_not_offered() {
    assert!(variants(&[f("m-Q8_0-00001-of-00003.gguf", 10), f("m-Q8_0-00003-of-00003.gguf", 5)]).is_empty());
}

#[test]
fn quant_names_are_read_in_the_styles_people_use() {
    for (name, label) in [("Qwen3-4B.Q4_K_M.gguf", "Q4_K_M"), ("qwen2.5-1.5b-instruct-fp16.gguf", "F16"), ("gpt-oss-20b-MXFP4.gguf", "MXFP4"), ("x.f16.gguf.Q4_K_M.gguf", "Q4_K_M"),
        ("granite-4.2-8b-Q5_0.gguf", "Q5_0"), ("model-q8_0.gguf", "Q8_0"), ("Phi-4.gguf", "GGUF"), ("m-TQ1_0.gguf", "TQ1_0"), ("tinyllama-Q3_K_L.gguf", "Q3_K_L")] {
        assert_eq!(variants(&[f(name, 1)])[0].label, label, "{name}");
    }
}

#[test]
fn reads_a_real_search_page() {
    let p = parse_search(include_str!("fixtures/search.json")).unwrap();
    assert!(!p.hits.is_empty() && p.total >= p.hits.len() as u64);
    assert!(p.hits[0].id.starts_with("hf-") && p.hits[0].repo.contains('/'));
    assert!(parse_search("<html>").is_err());
}

#[test]
fn a_real_plan_lists_only_files_that_can_be_downloaded() {
    let d = parse_plan(include_str!("fixtures/plan.json")).unwrap();
    assert_eq!(d.repo, "Qwen/Qwen3-4B-GGUF");
    assert_eq!(d.files.len(), 6);
    assert_eq!(d.variants.iter().map(|v| v.label.as_str()).collect::<Vec<_>>(), ["Q5_0", "Q5_K_M", "Q6_K", "Q8_0"]);
    let none = r#"{"artifactId":"a","artifact":{"repo":"o/r","licenseId":null},"files":[{"path":"m-Q4_0.gguf","sizeBytes":1,"sha256":"x","downloadUrl":false},{"path":"n-Q4_0.gguf","sizeBytes":1,"sha256":"x","downloadUrl":"https://evil.example/x"}],"reason":"Withheld."}"#;
    let d = parse_plan(none).unwrap();
    assert!(d.files.is_empty(), "withheld rows and other hosts are refused");
    assert_eq!(d.note.as_deref(), Some("Withheld."));
}
