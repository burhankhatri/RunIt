//! Fit timings from this machine, then predict any model from its shape.
use crate::gguf::Shape;
use crate::hardware::Machine;
use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Load {
    pub file_bytes: u64,
    /// Weight bytes read to produce one token.
    pub touched_bytes: f64,
    /// Parameters multiplied per prompt token.
    pub active_params: f64,
    pub layers: u32,
    pub kv_bytes_per_token: f64,
    pub context: u32,
    pub sliding_window: u32,
    /// False when only the file size was known.
    pub from_header: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Sample { pub touched_bytes: f64, pub active_params: f64, pub layers: u32, pub decode_s: f64, pub prompt_s: f64 }

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Fit {
    pub decode_s_per_byte: f64,
    pub decode_s_per_layer: f64,
    pub prompt_s_per_param: f64,
    pub prompt_s_per_layer: f64,
    /// Seconds per byte of KV read, per token.
    pub kv_s_per_byte: f64,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Calibration { pub gpu: Option<Fit>, pub cpu: Fit }

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Tier { Comfortable, Tight, Partial, TooBig }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Prediction {
    pub tier: Tier,
    pub need_bytes: u64,
    pub capacity_bytes: u64,
    pub tokens_per_s: f64,
    pub tokens_per_s_low: f64,
    pub tokens_per_s_high: f64,
    pub tokens_per_s_deep: f64,
    pub prompt_tokens_per_s: f64,
    /// Share of layers on the GPU.
    pub gpu_share: f64,
    pub context: u32,
    pub rough: bool,
}

/// Context assumed when stating memory needs.
pub const CONTEXT: u32 = 8192;
/// Depth assumed for the headline speed: a few messages in.
const SHALLOW: f64 = 512.0;
/// Scratch buffers and runtime, beyond weights and KV.
const OVERHEAD: u64 = 450 << 20;

/// Typical bits per weight, by `general.file_type`.
fn nominal_bits(file_type: u32) -> Option<f64> {
    Some(match file_type {
        1 => 16.0,
        2 => 4.6,
        7 => 8.5,
        10 | 21 => 3.2,
        11 => 3.6,
        12 => 4.0,
        13 => 4.3,
        14 => 4.6,
        15 => 4.9,
        16 => 5.6,
        17 => 5.7,
        18 => 6.6,
        _ => return None,
    })
}

pub fn load_from_shape(s: &Shape, file_bytes: u64) -> Load {
    let emb = s.embedding as f64;
    let mut attention = 0.0;
    let mut dense = 0.0;
    let mut experts = 0.0;
    let mut kv = 0.0;
    for (i, &kv_heads) in s.kv_heads.iter().enumerate() {
        let (k, v, h) = (s.key_length as f64, s.value_length as f64, kv_heads as f64);
        if kv_heads > 0 {
            attention += emb * (2.0 * s.heads as f64 * k + h * (k + v));
            kv += h * (k + v) * 2.0;
        } else {
            // Convolution and state-space blocks: roughly two square projections.
            attention += 2.0 * emb * emb;
        }
        let is_moe = s.experts > 0 && i >= s.dense_lead as usize;
        if is_moe {
            let width = if s.expert_feed_forward > 0 { s.expert_feed_forward } else { s.feed_forward[i] };
            experts += 3.0 * emb * width as f64 * s.experts as f64;
        } else {
            dense += 3.0 * emb * s.feed_forward[i] as f64;
        }
    }
    let table = emb * s.vocab as f64;
    let per_layer = s.per_layer_input as f64 * s.layers as f64 * s.vocab as f64;
    let body = attention + dense + experts;

    let tied = s.tied_embeddings.unwrap_or_else(|| {
        let explain = |params: f64| file_bytes as f64 * 8.0 / params;
        match s.file_type.and_then(nominal_bits) {
            Some(bits) => {
                let (t, u) = (explain(body + table + per_layer), explain(body + 2.0 * table + per_layer));
                (t - bits).abs() <= (u - bits).abs()
            }
            None => s.arch.starts_with("gemma"),
        }
    });
    let output = if tied { 0.0 } else { table };
    let total = body + table + output + per_layer;
    let used = if s.experts > 0 { s.experts_used.max(1) as f64 / s.experts as f64 } else { 1.0 };
    let idle = experts * (1.0 - used) + per_layer + if tied { 0.0 } else { table };
    Load {
        file_bytes,
        touched_bytes: file_bytes as f64 * (1.0 - idle / total),
        active_params: attention + dense + experts * used,
        layers: s.layers,
        kv_bytes_per_token: kv,
        context: if s.context > 0 { s.context } else { CONTEXT },
        sliding_window: s.sliding_window,
        from_header: true,
    }
}

/// When only the size is known. Assumes a dense model at about 4.9 bits per weight.
pub fn load_from_size(file_bytes: u64) -> Load {
    let gb = file_bytes as f64 / 1e9;
    let params = file_bytes as f64 * 8.0 / 4.9;
    Load {
        file_bytes,
        touched_bytes: file_bytes as f64,
        active_params: params,
        layers: (18.0 + 7.0 * gb.max(0.1).sqrt()) as u32,
        kv_bytes_per_token: 60_000.0 * gb.max(0.1).sqrt(),
        context: CONTEXT,
        sliding_window: 0,
        from_header: false,
    }
}

/// Least squares on relative error for `y = a*x + b*z`, both non-negative.
fn solve(rows: &[(f64, f64, f64)]) -> Option<(f64, f64)> {
    let (mut xx, mut xz, mut zz, mut xs, mut zs) = (0.0, 0.0, 0.0, 0.0, 0.0);
    for &(x, z, y) in rows {
        let (x, z) = (x / y, z / y);
        xx += x * x;
        xz += x * z;
        zz += z * z;
        xs += x;
        zs += z;
    }
    if xx <= 0.0 {
        return None;
    }
    let det = xx * zz - xz * xz;
    if det > 1e-9 * xx * zz {
        let (a, b) = ((xs * zz - zs * xz) / det, (zs * xx - xs * xz) / det);
        if a > 0.0 && b >= 0.0 {
            return Some((a, b));
        }
    }
    Some((xs / xx, 0.0))
}

/// `deep`: the same model at depth, the KV bytes it read per token, and its seconds per token.
pub fn fit(samples: &[Sample], deep: Option<(Sample, f64, f64)>) -> Option<Fit> {
    let ok: Vec<_> = samples.iter().filter(|s| s.decode_s.is_finite() && s.prompt_s.is_finite() && s.decode_s > 0.0 && s.prompt_s > 0.0).collect();
    if ok.len() < 2 {
        return None;
    }
    let (decode_s_per_byte, decode_s_per_layer) = solve(&ok.iter().map(|s| (s.touched_bytes, s.layers as f64, s.decode_s)).collect::<Vec<_>>())?;
    let (prompt_s_per_param, prompt_s_per_layer) = solve(&ok.iter().map(|s| (s.active_params, s.layers as f64, s.prompt_s)).collect::<Vec<_>>())?;
    let kv_s_per_byte = match deep {
        Some((base, kv_bytes, seconds)) if kv_bytes > 0.0 => ((seconds - base.decode_s) / kv_bytes).max(0.0),
        _ => decode_s_per_byte,
    };
    Some(Fit { decode_s_per_byte, decode_s_per_layer, prompt_s_per_param, prompt_s_per_layer, kv_s_per_byte })
}

fn kv_tokens(l: &Load, depth: f64) -> f64 {
    if l.sliding_window > 0 {
        // Most layers keep only the window; about one in six sees everything.
        let short = depth.min(l.sliding_window as f64);
        short * 5.0 / 6.0 + depth / 6.0
    } else {
        depth
    }
}

pub fn predict(l: &Load, m: &Machine, c: &Calibration) -> Prediction {
    let context = CONTEXT.min(l.context.max(512));
    let kv = l.kv_bytes_per_token * kv_tokens(l, context as f64);
    let need = l.file_bytes + kv as u64 + OVERHEAD;
    let capacity = m.capacity();
    let on_gpu = c.gpu.is_some() && m.gpu.is_some();
    let shared = m.gpu.as_ref().is_none_or(|g| g.integrated);
    let everything = if shared { m.ram_capacity() } else { capacity + m.ram_capacity() };

    let tier = if need as f64 <= capacity as f64 * 0.8 {
        Tier::Comfortable
    } else if need <= capacity {
        Tier::Tight
    } else if on_gpu && need <= everything {
        Tier::Partial
    } else {
        Tier::TooBig
    };
    let gpu_share = match tier {
        _ if !on_gpu => 0.0,
        Tier::Comfortable | Tier::Tight => 1.0,
        Tier::Partial => (capacity.saturating_sub(OVERHEAD) as f64 / (need - OVERHEAD) as f64).clamp(0.0, 1.0),
        Tier::TooBig => 0.0,
    };

    let decode = |f: &Fit, depth: f64| l.touched_bytes * f.decode_s_per_byte + l.layers as f64 * f.decode_s_per_layer + l.kv_bytes_per_token * kv_tokens(l, depth) * f.kv_s_per_byte;
    let prompt = |f: &Fit| l.active_params * f.prompt_s_per_param + l.layers as f64 * f.prompt_s_per_layer;
    let mix = |g: f64, cpu: f64| gpu_share * g + (1.0 - gpu_share) * cpu;
    let seconds = |depth: f64| match &c.gpu {
        Some(g) if on_gpu => mix(decode(g, depth), decode(&c.cpu, depth)),
        _ => decode(&c.cpu, depth),
    };
    let prompt_s = match &c.gpu {
        Some(g) if on_gpu => mix(prompt(g), prompt(&c.cpu)),
        _ => prompt(&c.cpu),
    };
    let rate = |s: f64| if tier == Tier::TooBig || s <= 0.0 { 0.0 } else { 1.0 / s };
    let speed = rate(seconds(SHALLOW));
    // Widths checked against real models; see docs/accuracy.md.
    let margin = match (l.from_header, tier) {
        (true, Tier::Comfortable | Tier::Tight) => 0.15,
        (true, _) => 0.30,
        (false, _) => 0.35,
    };
    Prediction {
        tier,
        need_bytes: need,
        capacity_bytes: capacity,
        tokens_per_s: speed,
        tokens_per_s_low: speed * (1.0 - margin),
        tokens_per_s_high: speed * (1.0 + margin),
        tokens_per_s_deep: rate(seconds(context as f64)),
        prompt_tokens_per_s: rate(prompt_s),
        gpu_share,
        context,
        rough: !l.from_header,
    }
}
