//! Turn predictions and ratings into a short list.
use crate::catalog::{Entry, Variant};
use crate::hardware::Machine;
use crate::predict::{load_from_size, predict, Calibration, Load, Prediction, Tier};
use crate::quality::{trust, Rating, Ratings};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice { pub id: String, pub repo: String, pub name: String, pub license: String, pub downloads: u64, pub rating: Option<Rating>, pub variant: Variant, pub prediction: Prediction }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pick {
    pub role: String,
    pub title: String,
    pub why: String,
    /// Other roles the same model won.
    pub also: Vec<String>,
    pub choice: Choice,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shortlist { pub picks: Vec<Pick>, pub ranked: Vec<Choice>, pub unrated: Vec<Choice>, pub ratings_date: String, pub ratings_source: String }

const MIN_BITS: f64 = 4.0;
/// Comfortably faster than people read.
pub const SMOOTH: f64 = 10.0;
pub const QUICK: f64 = 40.0;
const PATIENT: f64 = 6.0;
/// Headers are read for this many of the best-rated candidates.
const LOOK_CLOSER: usize = 28;

const UNRATED: usize = 8;

/// Weights in the model, from a name like `Qwen3-30B-A3B`. Not `E4B` or `8x7B`, which mean other things.
fn named_params(repo: &str) -> Option<f64> {
    repo.rsplit('/').next()?.to_lowercase().split(['-', '_', ' ']).find_map(|w| {
        let n = w.strip_suffix('b')?;
        n.starts_with(|c: char| c.is_ascii_digit()).then(|| n.parse::<f64>().ok())?.map(|n| n * 1e9)
    })
}

/// Repositories also ship small helper models beside the real one.
fn plausible(repo: &str, v: &Variant) -> bool {
    named_params(repo).is_none_or(|p| v.size as f64 > p * v.bits / 8.0 * 0.6)
}

/// The quant to offer: the finest that is still quick, else the smallest at four bits or better.
fn choose<'a>(repo: &str, vs: &'a [Variant], m: &Machine, c: &Calibration, load: &dyn Fn(&Variant) -> Load) -> Option<(&'a Variant, Prediction)> {
    let fair: Vec<_> = vs.iter().filter(|v| v.bits >= MIN_BITS && v.bits < 9.0 && plausible(repo, v)).map(|v| (v, predict(&load(v), m, c))).filter(|(_, p)| p.tier <= Tier::Tight).collect();
    let usual = fair.iter().find(|(v, _)| v.label == "Q4_K_M").or_else(|| fair.first()).cloned();
    let finer = fair.iter().filter(|(_, p)| p.tier == Tier::Comfortable && p.tokens_per_s >= QUICK).max_by(|a, b| a.0.bits.total_cmp(&b.0.bits)).cloned();
    match (usual, finer) {
        (Some(u), Some(f)) if f.0.bits > u.0.bits => Some(f),
        (u, f) => u.or(f),
    }
}

/// `shapes` returns each model's real load from its header, when that can be read.
pub fn shortlist(entries: &[Entry], ratings: &Ratings, m: &Machine, c: &Calibration, shapes: &dyn Fn(&[&Variant]) -> Vec<Option<Load>>) -> Shortlist {
    let by_size = |v: &Variant| load_from_size(v.size);
    // One entry per rated model, its own publisher first, then the most downloaded.
    let mut best: BTreeMap<&str, (&Entry, &Rating)> = BTreeMap::new();
    let mut others: Vec<&Entry> = Vec::new();
    for e in entries {
        let t = trust(&e.repo);
        // A publisher that offers nothing in range must not hide one that does.
        if t == 0 || e.kind != "llm" || choose(&e.repo, &e.variants, m, c, &by_size).is_none() {
            continue;
        }
        match ratings.find(&e.repo) {
            Some(r) => {
                if best.get(r.name.as_str()).is_none_or(|(o, _)| (t, e.downloads) > (trust(&o.repo), o.downloads)) {
                    best.insert(&r.name, (e, r));
                }
            }
            None if t == 2 => others.push(e),
            None => {}
        }
    }
    let mut all: Vec<(&Entry, Option<&Rating>)> = best.into_values().map(|(e, r)| (e, Some(r))).collect();
    all.sort_by_key(|(e, r)| (std::cmp::Reverse(r.map(|r| r.rating)), e.repo.clone()));
    all.truncate(LOOK_CLOSER);
    others.sort_by_key(|e| std::cmp::Reverse(e.downloads));
    all.extend(others.into_iter().take(UNRATED).map(|e| (e, None)));

    // Variants of one model share a shape, so one header per model is enough.
    let firsts: Vec<&Variant> = all.iter().map(|(e, _)| choose(&e.repo, &e.variants, m, c, &by_size).map_or(&e.variants[0], |x| x.0)).collect();
    let loads = shapes(&firsts);
    let (mut ranked, unrated): (Vec<Choice>, Vec<Choice>) = all
        .iter()
        .zip(firsts.iter().zip(loads))
        .filter_map(|((e, r), (first, load))| {
            let scaled = |v: &Variant| match &load {
                Some(l) => Load { file_bytes: v.size, touched_bytes: l.touched_bytes * v.size as f64 / first.size as f64, ..l.clone() },
                None => load_from_size(v.size),
            };
            let (v, p) = choose(&e.repo, &e.variants, m, c, &scaled)?;
            Some(Choice {
                id: e.id.clone(),
                repo: e.repo.clone(),
                name: r.map_or_else(|| crate::app::pretty(&e.repo), |r| r.name.clone()),
                license: e.license.clone(),
                downloads: e.downloads,
                rating: r.cloned(),
                variant: v.clone(),
                prediction: p,
            })
        })
        .filter(|c| c.prediction.tokens_per_s >= PATIENT)
        .partition(|c| c.rating.is_some());
    let overall = |c: &Choice| c.rating.as_ref().map_or(0, |r| r.rating);
    let coding = |c: &Choice| c.rating.as_ref().and_then(|r| r.coding).unwrap_or(0);
    ranked.sort_by_key(|c| std::cmp::Reverse(overall(c)));

    let top = |min: f64, comfy: bool, score: &dyn Fn(&Choice) -> u32| {
        ranked
            .iter()
            .filter(|c| c.prediction.tokens_per_s >= min && (!comfy || c.prediction.tier == Tier::Comfortable) && score(c) > 0)
            .max_by(|a, b| score(a).cmp(&score(b)).then(a.prediction.tokens_per_s.total_cmp(&b.prediction.tokens_per_s)))
    };
    let mut picks: Vec<Pick> = Vec::new();
    let mut add = |role: &str, title: &str, c: Option<&Choice>, why: &dyn Fn(&Choice) -> String| {
        let Some(c) = c else { return };
        match picks.iter_mut().find(|p| p.choice.name == c.name) {
            Some(p) => p.also.push(role.into()),
            None => picks.push(Pick { role: role.into(), title: title.into(), why: why(c), also: vec![], choice: c.clone() }),
        }
    };
    let smooth = top(SMOOTH, true, &overall);
    add("best", "Best for this computer", smooth.or_else(|| top(PATIENT, false, &overall)), &|c| {
        if c.prediction.tokens_per_s >= SMOOTH {
            format!("The highest-rated model that runs smoothly here, with {} to spare.", gb(c.prediction.capacity_bytes.saturating_sub(c.prediction.need_bytes)))
        } else {
            format!("The highest-rated model this computer can run, though slowly, at {:.0} tokens a second.", c.prediction.tokens_per_s)
        }
    });
    add("quick", "Quickest worth using", top(QUICK, true, &overall), &|c| format!("Writes faster than anyone reads, and rates {} with people who compared it blind.", overall(c)));
    add("code", "Best for code", top(SMOOTH, true, &coding), &|c| format!("Rated {} on coding questions, the highest of what runs smoothly here.", coding(c)));
    add("smart", "Smartest it can hold", top(PATIENT, false, &overall), &|c| {
        if c.prediction.tier == Tier::Tight {
            "The strongest model that still fits. Close heavy apps first.".into()
        } else {
            format!("The strongest model that fits. Slower, at {:.0} tokens a second.", c.prediction.tokens_per_s)
        }
    });
    Shortlist { picks, ranked, unrated, ratings_date: ratings.date.clone(), ratings_source: ratings.source.clone() }
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64)
}
