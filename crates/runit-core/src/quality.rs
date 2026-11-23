//! Human-preference ratings, matched to repositories by exact name.
use serde::{Deserialize, Serialize};
use std::collections::BTreeSet;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Rating {
    pub name: String,
    pub rating: u32,
    pub votes: u32,
    pub coding: Option<u32>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct Ratings {
    pub source: String,
    pub date: String,
    models: Vec<Rating>,
}

/// Words that describe packaging or the vendor, not which model it is.
const SOFT: &[&str] = &[
    "gguf", "it", "instruct", "chat", "text", "imat", "imatrix", "i1", "qat", "hf", "meta", "nvidia", "google", "openai", "microsoft", "ibm", "k", "m", "s", "l", "0", "1", "bf16", "fp16", "f16", "fp8",
    "nvfp4", "mxfp4", "high", "max", "ud",
];

fn words(s: &str) -> Vec<String> {
    s.to_lowercase().split(|c: char| !c.is_ascii_alphanumeric() && c != '.').map(|w| w.trim_matches('.').to_string()).filter(|w| !w.is_empty()).collect()
}

fn is_quant(w: &str) -> bool {
    let d = w.strip_prefix("iq").or_else(|| w.strip_prefix('q'));
    d.is_some_and(|d| d.len() == 1 && d.as_bytes()[0].is_ascii_digit())
}

fn is_size(w: &str) -> bool {
    w.strip_suffix('b').is_some_and(|n| !n.is_empty() && n.chars().all(|c| c.is_ascii_digit() || c == '.'))
}

fn core(s: &str) -> BTreeSet<String> {
    words(s).into_iter().filter(|w| !SOFT.contains(&w.as_str()) && !is_quant(w)).collect()
}

fn tuned(s: &str) -> bool {
    words(s).iter().any(|w| matches!(w.as_str(), "it" | "instruct" | "chat"))
}

impl Ratings {
    pub fn bundled() -> Ratings {
        serde_json::from_str(include_str!("../../../data/quality.json")).expect("bundled ratings parse")
    }

    /// The rating of exactly this model, or nothing. Fine-tunes inherit nothing.
    pub fn find(&self, repo: &str) -> Option<&Rating> {
        let name = repo.rsplit('/').next()?;
        let have = core(name);
        self.models
            .iter()
            .filter(|r| {
                // An instruct model's rating says nothing about the raw base model.
                if tuned(&r.name) && !tuned(name) {
                    return false;
                }
                let want = core(&r.name);
                want == have || (!want.iter().any(|w| is_size(w)) && want.len() >= 3 && have.iter().filter(|w| !is_size(w)).eq(want.iter()))
            })
            .max_by_key(|r| r.votes)
    }
}

/// 2: the model's own publisher. 1: a well-known repackager. 0: anyone else.
pub fn trust(repo: &str) -> u8 {
    let owner = repo.split('/').next().unwrap_or("").to_lowercase();
    match owner.as_str() {
        "qwen" | "google" | "ibm-granite" | "meta-llama" | "mistralai" | "microsoft" | "nvidia" | "openai" | "liquidai" | "allenai" | "deepseek-ai" | "zai-org" | "tencent" | "cohereforai" | "coherelabs"
        | "xiaomimimo" | "moonshotai" | "minimaxai" | "stepfun-ai" | "inclusionai" | "huggingfacetb" | "thinkingmachines" => 2,
        "unsloth" | "ggml-org" | "bartowski" | "lmstudio-community" | "maziyarpanahi" | "mradermacher" | "thebloke" | "second-state" | "devquasar" | "legraphista" => 1,
        _ => 0,
    }
}
