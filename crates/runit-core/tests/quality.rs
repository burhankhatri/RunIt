use runit_core::quality::*;

#[test]
fn packaging_words_do_not_hide_a_match() {
    let r = Ratings::bundled();
    for (repo, name) in [
        ("unsloth/Llama-3.2-3B-Instruct-GGUF", "Llama 3.2 3B Instruct"), ("ibm-granite/granite-4.2-8b-GGUF", "Granite 4.2 8B"),
        ("unsloth/Qwen3-30B-A3B-Instruct-2507-GGUF", "Qwen3-30B-A3B-Instruct (2507)"), ("MaziyarPanahi/Qwen3-30B-A3B-GGUF", "Qwen3-30B-A3B"),
        ("lmstudio-community/gemma-3n-E4B-it-text-GGUF", "Gemma 3n E4B IT"), ("ggml-org/gpt-oss-20b-GGUF", "gpt-oss-20b"),
        ("bartowski/Mistral-Small-3.2-24B-Instruct-2506-GGUF", "Mistral Small 3.2 (2506)"), ("MaziyarPanahi/Meta-Llama-3-8B-Instruct-GGUF", "Llama 3 8B Instruct"),
        ("google/gemma-3-12b-it-qat-q4_0-gguf", "Gemma 3 12B IT"), ("microsoft/phi-4-gguf", "Phi-4"),
    ] {
        assert_eq!(r.find(repo).map(|r| r.name.as_str()), Some(name), "{repo}");
    }
}

#[test]
fn different_models_do_not_borrow_a_rating() {
    let r = Ratings::bundled();
    for repo in ["AnkitAI/Parable-Qwen3-32B-Claude-Fable-5-GGUF", "empero-ai/Qwen3.8-27B-Distill-GGUF", "Jiunsong/gemma-4-26B-A4B-it-uncensored-GGUF", "x/Huihui-gemma-4-26B-A4B-it-abliterated-GGUF",
        "unsloth/Llama-3.2-3B-GGUF", "unsloth/Qwen3-Coder-30B-A3B-Instruct-GGUF", "microsoft/phi-4-mini-instruct-GGUF", "mradermacher/AgriGemma-3n-E4B-it-GGUF", "AtomicChat/gemma-4-31B-it-assistant-GGUF", "Qwen/Qwen3-4B-GGUF"] {
        assert_eq!(r.find(repo), None, "{repo}");
    }
}

#[test]
fn publishers_rank_original_then_known_then_unknown() {
    assert_eq!(trust("ibm-granite/granite-4.2-8b-GGUF"), 2);
    assert_eq!(trust("unsloth/x-GGUF"), 1);
    assert_eq!(trust("somebody/x-GGUF"), 0);
}
