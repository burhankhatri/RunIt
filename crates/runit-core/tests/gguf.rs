use runit_core::gguf::*;

struct Buf(Vec<u8>, u64);
impl Buf {
    fn new() -> Self { Buf(vec![], 0) }
    fn s(v: &mut Vec<u8>, x: &str) { v.extend((x.len() as u64).to_le_bytes()); v.extend(x.as_bytes()); }
    fn u32(mut self, k: &str, v: u32) -> Self { Self::s(&mut self.0, k); self.0.extend(4u32.to_le_bytes()); self.0.extend(v.to_le_bytes()); self.1 += 1; self }
    fn str(mut self, k: &str, v: &str) -> Self { Self::s(&mut self.0, k); self.0.extend(8u32.to_le_bytes()); Self::s(&mut self.0, v); self.1 += 1; self }
    fn arr_i32(mut self, k: &str, v: &[i32]) -> Self {
        Self::s(&mut self.0, k); self.0.extend(9u32.to_le_bytes()); self.0.extend(5u32.to_le_bytes());
        self.0.extend((v.len() as u64).to_le_bytes()); for x in v { self.0.extend(x.to_le_bytes()); } self.1 += 1; self }
    fn tokens(mut self, n: usize) -> Self {
        Self::s(&mut self.0, "tokenizer.ggml.tokens"); self.0.extend(9u32.to_le_bytes()); self.0.extend(8u32.to_le_bytes());
        self.0.extend((n as u64).to_le_bytes()); for i in 0..n { Self::s(&mut self.0, &format!("tok{i}")); } self.1 += 1; self }
    fn done(self, tensors: &[&str]) -> Vec<u8> {
        let mut o = b"GGUF".to_vec(); o.extend(3u32.to_le_bytes()); o.extend((tensors.len() as u64).to_le_bytes()); o.extend(self.1.to_le_bytes()); o.extend(self.0);
        for t in tensors { Self::s(&mut o, t); o.extend(1u32.to_le_bytes()); o.extend(8u64.to_le_bytes()); o.extend(0u32.to_le_bytes()); o.extend(0u64.to_le_bytes()); }
        o
    }
}

fn llama() -> Buf {
    Buf::new().str("general.architecture", "llama").u32("llama.block_count", 28).u32("llama.embedding_length", 3072)
        .u32("llama.feed_forward_length", 8192).u32("llama.attention.head_count", 24).u32("llama.attention.head_count_kv", 8)
        .u32("llama.context_length", 131072).u32("llama.vocab_size", 128256).u32("general.file_type", 15)
}

#[test]
fn reads_a_dense_model() {
    let s = read_header(&llama().tokens(5).done(&["token_embd.weight", "output.weight"])).unwrap();
    assert_eq!((s.arch.as_str(), s.layers, s.embedding, s.vocab), ("llama", 28, 3072, 128256));
    assert_eq!(s.kv_heads, vec![8; 28]);
    assert_eq!((s.key_length, s.value_length), (128, 128));
    assert_eq!(s.feed_forward, vec![8192; 28]);
    assert_eq!(s.file_type, Some(15));
    assert_eq!(s.tied_embeddings, Some(false));
    assert_eq!(s.experts, 0);
}

#[test]
fn spots_tied_embeddings() {
    let s = read_header(&llama().done(&["token_embd.weight", "output_norm.weight"])).unwrap();
    assert_eq!(s.tied_embeddings, Some(true));
}

#[test]
fn reads_a_header_cut_in_the_middle_of_the_vocabulary() {
    let full = llama().tokens(50_000).done(&["token_embd.weight"]);
    let s = read_header(&full[..4096]).unwrap();
    assert_eq!((s.layers, s.vocab, s.tied_embeddings), (28, 128256, None));
}

#[test]
fn takes_vocab_from_the_token_array_length_when_unstated() {
    let b = Buf::new().str("general.architecture", "qwen2").u32("qwen2.block_count", 2).u32("qwen2.embedding_length", 64)
        .u32("qwen2.feed_forward_length", 128).u32("qwen2.attention.head_count", 4).tokens(9000).done(&[]);
    let s = read_header(&b[..2000]).unwrap();
    assert_eq!(s.vocab, 9000);
    assert_eq!(s.kv_heads, vec![4, 4]);
}

#[test]
fn reads_per_layer_arrays_and_experts() {
    let b = Buf::new().str("general.architecture", "lfm2moe").u32("lfm2moe.block_count", 3).u32("lfm2moe.embedding_length", 1024)
        .arr_i32("lfm2moe.feed_forward_length", &[100, 200, 300]).u32("lfm2moe.attention.head_count", 16)
        .arr_i32("lfm2moe.attention.head_count_kv", &[0, 8, 0]).u32("lfm2moe.expert_count", 32).u32("lfm2moe.expert_used_count", 4)
        .u32("lfm2moe.expert_feed_forward_length", 768).u32("lfm2moe.leading_dense_block_count", 1).done(&[]);
    let s = read_header(&b).unwrap();
    assert_eq!(s.kv_heads, vec![0, 8, 0]);
    assert_eq!(s.feed_forward, vec![100, 200, 300]);
    assert_eq!((s.experts, s.experts_used, s.expert_feed_forward, s.dense_lead), (32, 4, 768, 1));
}

#[test]
fn rejects_what_is_not_gguf_or_lacks_a_shape() {
    assert!(read_header(b"PK\x03\x04aaaaaaaaaaaaaaaaaaaaaaaa").is_err());
    assert!(read_header(b"GGUF").is_err());
    assert!(read_header(&Buf::new().str("general.architecture", "llama").done(&[])).is_err());
    // A hostile length must not allocate or panic.
    let mut evil = b"GGUF".to_vec(); evil.extend(3u32.to_le_bytes()); evil.extend(0u64.to_le_bytes()); evil.extend(u64::MAX.to_le_bytes()); evil.extend(u64::MAX.to_le_bytes());
    assert!(read_header(&evil).is_err());
}

#[test]
fn synthetic_models_read_back_with_the_shape_they_were_given() {
    let dir = std::env::temp_dir().join(format!("runit-gguf-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join("a.gguf");
    let syn = Synthetic { layers: 4, embedding: 256, feed_forward: 1024, heads: 4, kv_heads: 2, vocab: 1000 };
    let size = write_synthetic(&p, &syn).unwrap();
    let bytes = std::fs::read(&p).unwrap();
    assert_eq!(size, bytes.len() as u64);
    let s = read_header(&bytes).unwrap();
    assert_eq!((s.layers, s.embedding, s.vocab, s.heads), (4, 256, 1000, 4));
    assert_eq!(s.kv_heads, vec![2; 4]);
    assert_eq!(s.tied_embeddings, Some(false));
    // Weights vary; a constant file could be compressed or cached by a driver.
    let tail = &bytes[bytes.len() - 4096..];
    assert!(tail.iter().collect::<std::collections::HashSet<_>>().len() > 64);
    std::fs::remove_dir_all(dir).unwrap();
}
