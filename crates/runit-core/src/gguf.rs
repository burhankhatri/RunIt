//! Reading GGUF headers (also cut-off ones) and writing random-weight test models.
use std::collections::HashMap;
use std::io::{self, Write};
use std::path::Path;

#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    pub arch: String,
    pub layers: u32,
    pub embedding: u32,
    pub vocab: u32,
    pub heads: u32,
    /// KV heads per layer.
    pub kv_heads: Vec<u32>,
    pub key_length: u32,
    pub value_length: u32,
    /// Feed-forward width per layer.
    pub feed_forward: Vec<u32>,
    pub experts: u32,
    pub experts_used: u32,
    pub expert_feed_forward: u32,
    pub dense_lead: u32,
    pub per_layer_input: u32,
    pub context: u32,
    pub sliding_window: u32,
    pub file_type: Option<u32>,
    pub chat_template: bool,
    /// Set when tensor descriptions were readable (whole header present).
    pub tied_embeddings: Option<bool>,
}

/// A metadata value we care about. Strings inside arrays are skipped, only the count is kept.
enum Value {
    Int(i64),
    Text(String),
    Ints(Vec<i64>),
    Other,
}

struct Cursor<'a> {
    b: &'a [u8],
    at: usize,
}

/// The header ended before this value did. Expected for ranged reads.
struct Cut;

impl<'a> Cursor<'a> {
    fn take(&mut self, n: usize) -> Result<&'a [u8], Cut> {
        let end = self.at.checked_add(n).ok_or(Cut)?;
        let s = self.b.get(self.at..end).ok_or(Cut)?;
        self.at = end;
        Ok(s)
    }
    fn u32(&mut self) -> Result<u32, Cut> {
        Ok(u32::from_le_bytes(self.take(4)?.try_into().unwrap()))
    }
    fn u64(&mut self) -> Result<u64, Cut> {
        Ok(u64::from_le_bytes(self.take(8)?.try_into().unwrap()))
    }
    fn len(&mut self) -> Result<usize, Cut> {
        usize::try_from(self.u64()?).map_err(|_| Cut)
    }
    fn text(&mut self) -> Result<String, Cut> {
        let n = self.len()?;
        Ok(String::from_utf8_lossy(self.take(n)?).into_owned())
    }
    fn int(&mut self, ty: u32) -> Result<Option<i64>, Cut> {
        let v = match ty {
            0 | 7 => self.take(1)?[0] as i64,
            1 => self.take(1)?[0] as i8 as i64,
            2 => u16::from_le_bytes(self.take(2)?.try_into().unwrap()) as i64,
            3 => i16::from_le_bytes(self.take(2)?.try_into().unwrap()) as i64,
            4 => self.u32()? as i64,
            5 => self.u32()? as i32 as i64,
            10 | 11 => self.u64()? as i64,
            6 => {
                self.take(4)?;
                return Ok(None);
            }
            12 => {
                self.take(8)?;
                return Ok(None);
            }
            _ => return Err(Cut),
        };
        Ok(Some(v))
    }
    /// Second field: for arrays, the element count, known even when the contents are cut off.
    fn value(&mut self, ty: u32, count: &mut Option<u64>) -> Result<Value, Cut> {
        match ty {
            8 => Ok(Value::Text(self.text()?)),
            9 => {
                let inner = self.u32()?;
                let n = self.u64()?;
                *count = Some(n);
                if inner == 8 {
                    for _ in 0..n {
                        let l = self.len()?;
                        self.take(l)?;
                    }
                    return Ok(Value::Other);
                }
                let mut ints = Vec::new();
                for _ in 0..n {
                    if let Some(v) = self.int(inner)? {
                        // Per-layer arrays are short; token scores are not needed.
                        if ints.len() < 4096 {
                            ints.push(v);
                        }
                    }
                }
                Ok(Value::Ints(ints))
            }
            _ => Ok(self.int(ty)?.map_or(Value::Other, Value::Int)),
        }
    }
}

pub fn read_header(bytes: &[u8]) -> Result<Shape, String> {
    let mut c = Cursor { b: bytes, at: 0 };
    let short = |_| "file is too short to be a GGUF".to_string();
    if c.take(4).map_err(short)? != b"GGUF" {
        return Err("not a GGUF file".into());
    }
    let version = c.u32().map_err(short)?;
    if !(2..=3).contains(&version) {
        return Err(format!("unsupported GGUF version {version}"));
    }
    let n_tensors = c.u64().map_err(short)?;
    let n_kv = c.u64().map_err(short)?;

    let mut map: HashMap<String, Value> = HashMap::new();
    let mut tokens = None;
    let mut complete = true;
    for _ in 0..n_kv {
        let mut count = None;
        let mut entry = || -> Result<(String, Value), (Option<String>, Cut)> {
            let key = c.text().map_err(|e| (None, e))?;
            let ty = c.u32().map_err(|e| (Some(key.clone()), e))?;
            match c.value(ty, &mut count) {
                Ok(v) => Ok((key, v)),
                Err(e) => Err((Some(key), e)),
            }
        };
        match entry() {
            Ok((key, v)) => {
                if key == "tokenizer.ggml.tokens" {
                    tokens = count;
                }
                map.insert(key, v);
            }
            Err((key, _)) => {
                if key.as_deref() == Some("tokenizer.ggml.tokens") {
                    tokens = count;
                }
                complete = false;
                break;
            }
        }
    }

    let mut tied = None;
    if complete {
        let mut has_output = false;
        let mut ok = true;
        for _ in 0..n_tensors {
            let mut tensor = || -> Result<bool, Cut> {
                let name = c.text()?;
                let dims = c.u32()?;
                c.take((dims as usize).checked_mul(8).ok_or(Cut)? + 4 + 8)?;
                Ok(name == "output.weight")
            };
            match tensor() {
                Ok(o) => has_output |= o,
                Err(_) => {
                    ok = false;
                    break;
                }
            }
        }
        if ok {
            tied = Some(!has_output);
        }
    }

    let arch = match map.get("general.architecture") {
        Some(Value::Text(t)) => t.clone(),
        _ => return Err("no architecture in header".into()),
    };
    let int = |k: &str| match map.get(&format!("{arch}.{k}")) {
        Some(Value::Int(v)) => u32::try_from(*v).ok(),
        Some(Value::Ints(v)) => v.iter().max().and_then(|m| u32::try_from(*m).ok()),
        _ => None,
    };
    let layers = int("block_count").filter(|n| (1..=1024).contains(n)).ok_or("no layer count in header")?;
    let embedding = int("embedding_length").filter(|n| *n > 0).ok_or("no embedding size in header")?;
    let per_layer = |k: &str, default: u32| match map.get(&format!("{arch}.{k}")) {
        Some(Value::Int(v)) => vec![u32::try_from(*v).unwrap_or(default); layers as usize],
        Some(Value::Ints(v)) if v.len() >= layers as usize => {
            v[..layers as usize].iter().map(|x| u32::try_from(*x).unwrap_or(0)).collect()
        }
        _ => vec![default; layers as usize],
    };
    let heads = int("attention.head_count").unwrap_or(0);
    let head_size = if heads > 0 { embedding / heads } else { 0 };
    let vocab = int("vocab_size").or(tokens.and_then(|n| u32::try_from(n).ok())).unwrap_or(0);
    Ok(Shape {
        layers,
        embedding,
        vocab,
        heads,
        kv_heads: per_layer("attention.head_count_kv", heads),
        key_length: int("attention.key_length").unwrap_or(head_size),
        value_length: int("attention.value_length").unwrap_or(head_size),
        feed_forward: per_layer("feed_forward_length", 0),
        experts: int("expert_count").unwrap_or(0),
        experts_used: int("expert_used_count").unwrap_or(0),
        expert_feed_forward: int("expert_feed_forward_length").unwrap_or(0),
        dense_lead: int("leading_dense_block_count").unwrap_or(0),
        per_layer_input: int("embedding_length_per_layer_input").unwrap_or(0),
        context: int("context_length").unwrap_or(0),
        sliding_window: int("attention.sliding_window").unwrap_or(0),
        file_type: match map.get("general.file_type") {
            Some(Value::Int(v)) => u32::try_from(*v).ok(),
            _ => None,
        },
        chat_template: map.contains_key("tokenizer.chat_template"),
        tied_embeddings: tied,
        arch,
    })
}

#[derive(Clone, Copy, Debug)]
pub struct Synthetic {
    pub layers: u32,
    pub embedding: u32,
    pub feed_forward: u32,
    pub heads: u32,
    pub kv_heads: u32,
    pub vocab: u32,
}

const F32: u32 = 0;
const Q4_K: u32 = 12;
const Q6_K: u32 = 14;
const ALIGN: u64 = 32;

fn tensor_bytes(ty: u32, dims: &[u64]) -> u64 {
    let n: u64 = dims.iter().product();
    match ty {
        F32 => n * 4,
        Q4_K => n / 256 * 144,
        Q6_K => n / 256 * 210,
        _ => unreachable!("synthetic models use three types"),
    }
}

/// Valid quantized blocks with noisy weights and a small fixed scale, so no NaN, infinity or
/// denormal can change the timing.
fn noise(block: usize, half_floats: &[usize]) -> Vec<u8> {
    let mut state = 0x9E37_79B9_7F4A_7C15u64;
    let mut v = Vec::with_capacity(block * 6000);
    for _ in 0..6000 {
        let start = v.len();
        while v.len() < start + block {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            v.extend_from_slice(&state.to_le_bytes());
        }
        v.truncate(start + block);
        for at in half_floats {
            v[start + at..start + at + 2].copy_from_slice(&0x1401u16.to_le_bytes());
        }
    }
    v
}

/// Write a llama-architecture model with random Q4_K weights. Returns the file size.
pub fn write_synthetic(path: &Path, s: &Synthetic) -> io::Result<u64> {
    let (emb, ff, vocab) = (s.embedding as u64, s.feed_forward as u64, s.vocab as u64);
    let kv = emb / s.heads as u64 * s.kv_heads as u64;
    let mut tensors: Vec<(String, Vec<u64>, u32)> = vec![
        ("token_embd.weight".into(), vec![emb, vocab], Q4_K),
        ("output_norm.weight".into(), vec![emb], F32),
        ("output.weight".into(), vec![emb, vocab], Q6_K),
    ];
    for i in 0..s.layers {
        for (name, dims, ty) in [
            ("attn_norm", vec![emb], F32),
            ("ffn_norm", vec![emb], F32),
            ("attn_q", vec![emb, emb], Q4_K),
            ("attn_k", vec![emb, kv], Q4_K),
            ("attn_v", vec![emb, kv], Q4_K),
            ("attn_output", vec![emb, emb], Q4_K),
            ("ffn_gate", vec![emb, ff], Q4_K),
            ("ffn_down", vec![ff, emb], Q4_K),
            ("ffn_up", vec![emb, ff], Q4_K),
        ] {
            tensors.push((format!("blk.{i}.{name}.weight"), dims, ty));
        }
    }

    let mut h = Vec::new();
    let text = |h: &mut Vec<u8>, t: &str| {
        h.extend((t.len() as u64).to_le_bytes());
        h.extend(t.as_bytes());
    };
    let strings = [
        ("general.architecture", "llama"),
        ("general.name", "RunIt benchmark (random weights)"),
        ("tokenizer.ggml.model", "no_vocab"),
    ];
    let ints = [
        ("general.file_type", 15),
        ("llama.block_count", s.layers),
        ("llama.context_length", 8192),
        ("llama.embedding_length", s.embedding),
        ("llama.feed_forward_length", s.feed_forward),
        ("llama.attention.head_count", s.heads),
        ("llama.attention.head_count_kv", s.kv_heads),
        ("llama.rope.dimension_count", s.embedding / s.heads),
        ("llama.vocab_size", s.vocab),
    ];
    h.extend(b"GGUF");
    h.extend(3u32.to_le_bytes());
    h.extend((tensors.len() as u64).to_le_bytes());
    h.extend((strings.len() as u64 + ints.len() as u64 + 1).to_le_bytes());
    for (k, v) in strings {
        text(&mut h, k);
        h.extend(8u32.to_le_bytes());
        text(&mut h, v);
    }
    for (k, v) in ints {
        text(&mut h, k);
        h.extend(4u32.to_le_bytes());
        h.extend(v.to_le_bytes());
    }
    text(&mut h, "llama.attention.layer_norm_rms_epsilon");
    h.extend(6u32.to_le_bytes());
    h.extend(1e-5f32.to_le_bytes());

    let mut offset = 0u64;
    for (name, dims, ty) in &tensors {
        text(&mut h, name);
        h.extend((dims.len() as u32).to_le_bytes());
        for d in dims {
            h.extend(d.to_le_bytes());
        }
        h.extend(ty.to_le_bytes());
        h.extend(offset.to_le_bytes());
        offset += tensor_bytes(*ty, dims).next_multiple_of(ALIGN);
    }
    h.resize(h.len().next_multiple_of(ALIGN as usize), 0);

    let q4 = noise(144, &[140, 142]);
    let q6 = noise(210, &[208]);
    let one: Vec<u8> = 1.0f32.to_le_bytes().repeat(1024);
    let mut f = io::BufWriter::with_capacity(1 << 20, std::fs::File::create(path)?);
    f.write_all(&h)?;
    for (_, dims, ty) in &tensors {
        let src = match *ty {
            F32 => &one,
            Q4_K => &q4,
            _ => &q6,
        };
        let bytes = tensor_bytes(*ty, dims);
        let mut left = bytes as usize;
        while left > 0 {
            let n = left.min(src.len());
            f.write_all(&src[..n])?;
            left -= n;
        }
        f.write_all(&[0u8; ALIGN as usize][..(bytes.next_multiple_of(ALIGN) - bytes) as usize])?;
    }
    f.flush()?;
    Ok(h.len() as u64 + offset)
}
