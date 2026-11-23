//! Turn predictions and ratings into a short list.
use crate::catalog::{Entry, Variant};
use crate::hardware::Machine;
use crate::predict::{load_from_size, predict, Calibration, Load, Prediction, Tier};
use crate::quality::{trust, Rating, Ratings};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Choice { pub id: String, pub repo: String, pub name: String, pub license: String, pub rating: Rating, pub variant: Variant, pub prediction: Prediction }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Pick { pub role: String, pub title: String, pub why: String, pub choice: Choice }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Shortlist { pub picks: Vec<Pick>, pub ranked: Vec<Choice>, pub ratings_date: String, pub ratings_source: String }

const MIN_BITS: f64 = 4.0;
/// Fast enough to read along with comfortably.
pub const SMOOTH: f64 = 15.0;
pub const QUICK: f64 = 40.0;
const PATIENT: f64 = 6.0;
/// A finer quant must still be this fast to replace a smaller one.
const FINER: f64 = 30.0;
/// Headers are read for this many of the best-rated candidates.
const LOOK_CLOSER: usize = 28;

/// The quant to offer: the finest that stays quick, else the smallest at four bits or better.
fn choose<'a>(vs: &'a [Variant], m: &Machine, c: &Calibration, load: &dyn Fn(&Variant) -> Load) -> Option<(&'a Variant, Prediction)> {
    let fair: Vec<_> = vs.iter().filter(|v| v.bits >= MIN_BITS && v.bits < 9.0).map(|v| (v, predict(&load(v), m, c))).filter(|(_, p)| p.tier <= Tier::Tight).collect();
    let usual = fair.iter().find(|(v, _)| v.label == "Q4_K_M").or_else(|| fair.first()).cloned();
    let finer = fair.iter().filter(|(_, p)| p.tier == Tier::Comfortable && p.tokens_per_s >= FINER).max_by(|a, b| a.0.bits.total_cmp(&b.0.bits)).cloned();
    match (usual, finer) {
        (Some(u), Some(f)) if f.0.bits > u.0.bits => Some(f),
        (u, f) => u.or(f),
    }
}

/// `shapes` returns each model's real load from its header, when that can be read.
pub fn shortlist(entries: &[Entry], ratings: &Ratings, m: &Machine, c: &Calibration, shapes: &dyn Fn(&[&Variant]) -> Vec<Option<Load>>) -> Shortlist {
    // One entry per rated model, its own publisher first, then the most downloaded.
    let mut best: BTreeMap<&str, (&Entry, &Rating)> = BTreeMap::new();
    for e in entries {
        let t = trust(&e.repo);
        if t == 0 || e.kind != "llm" {
            continue;
        }
        let Some(r) = ratings.find(&e.repo) else { continue };
        // A publisher that offers nothing in range must not hide one that does.
        if choose(&e.variants, m, c, &|v| load_from_size(v.size)).is_none() {
            continue;
        }
        let better = best.get(r.name.as_str()).is_none_or(|(o, _)| (t, e.downloads) > (trust(&o.repo), o.downloads));
        if better {
            best.insert(&r.name, (e, r));
        }
    }
    let mut all: Vec<_> = best.into_values().collect();
    all.sort_by_key(|(e, r)| (std::cmp::Reverse(r.rating), e.repo.clone()));
    all.truncate(LOOK_CLOSER);

    // Variants of one model share a shape, so one header per model is enough.
    let firsts: Vec<&Variant> = all.iter().map(|(e, _)| choose(&e.variants, m, c, &|v| load_from_size(v.size)).map_or(&e.variants[0], |x| x.0)).collect();
    let loads = shapes(&firsts);
    let mut ranked: Vec<Choice> = all
        .iter()
        .zip(firsts.iter().zip(loads))
        .filter_map(|((e, r), (first, load))| {
            let scaled = |v: &Variant| match &load {
                Some(l) => Load { file_bytes: v.size, touched_bytes: l.touched_bytes * v.size as f64 / first.size as f64, ..l.clone() },
                None => load_from_size(v.size),
            };
            let (v, p) = choose(&e.variants, m, c, &scaled)?;
            Some(Choice { id: e.id.clone(), repo: e.repo.clone(), name: r.name.clone(), license: e.license.clone(), rating: (*r).clone(), variant: v.clone(), prediction: p })
        })
        .filter(|c| c.prediction.tokens_per_s >= PATIENT)
        .collect();
    ranked.sort_by_key(|c| std::cmp::Reverse(c.rating.rating));

    let top = |min: f64, comfy: bool, score: &dyn Fn(&Choice) -> u32| {
        ranked
            .iter()
            .filter(|c| c.prediction.tokens_per_s >= min && (!comfy || c.prediction.tier == Tier::Comfortable) && score(c) > 0)
            .max_by(|a, b| score(a).cmp(&score(b)).then(a.prediction.tokens_per_s.total_cmp(&b.prediction.tokens_per_s)))
    };
    let overall = |c: &Choice| c.rating.rating;
    let coding = |c: &Choice| c.rating.coding.unwrap_or(0);
    let mut picks: Vec<Pick> = Vec::new();
    let mut add = |role: &str, title: &str, c: Option<&Choice>, why: &dyn Fn(&Choice) -> String| {
        if let Some(c) = c.filter(|c| picks.iter().all(|p| p.choice.name != c.name)) {
            picks.push(Pick { role: role.into(), title: title.into(), why: why(c), choice: c.clone() });
        }
    };
    let smooth = top(SMOOTH, true, &overall);
    add("best", "Best for this computer", smooth.or_else(|| top(PATIENT, false, &overall)), &|c| {
        format!("The highest-rated model that runs smoothly here, with {} to spare.", gb(c.prediction.capacity_bytes.saturating_sub(c.prediction.need_bytes)))
    });
    add("quick", "Quickest worth using", top(QUICK, true, &overall), &|c| format!("Writes faster than anyone reads, and rates {} with people who compared it blind.", c.rating.rating));
    add("code", "Best for code", top(SMOOTH, true, &coding), &|c| format!("Rated {} on coding questions, the highest of what runs smoothly here.", coding(c)));
    add("smart", "Smartest it can hold", top(PATIENT, false, &overall), &|c| {
        if c.prediction.tier == Tier::Tight {
            "The strongest model that still fits. Close heavy apps first.".into()
        } else {
            format!("The strongest model that fits. Slower, at {:.0} tokens a second.", c.prediction.tokens_per_s)
        }
    });
    Shortlist { picks, ranked, ratings_date: ratings.date.clone(), ratings_source: ratings.source.clone() }
}

fn gb(bytes: u64) -> String {
    format!("{:.1} GB", bytes as f64 / (1u64 << 30) as f64)
}
