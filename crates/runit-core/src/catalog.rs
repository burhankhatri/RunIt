//! Hugging Bay: the daily snapshot, live search and file plans.
use crate::gguf::{read_header, Shape};
use crate::http::{self, encode};
use flate2::read::GzDecoder;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeMap;
use std::io::{BufRead, BufReader, Read};
use std::path::Path;

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FileRow { pub path: String, pub size: u64, pub sha256: String, pub url: String }

/// One downloadable choice: a single file, or all shards of one.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Variant { pub label: String, pub bits: f64, pub size: u64, pub files: Vec<FileRow> }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Entry { pub id: String, pub repo: String, pub kind: String, pub license: String, pub downloads: u64, pub variants: Vec<Variant> }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Hit { pub id: String, pub repo: String, pub kind: String, pub license: String, pub downloads: u64, pub formats: Vec<String>, pub size: Option<u64>, pub ready: Option<bool> }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Page { pub hits: Vec<Hit>, pub total: u64, pub next: Option<String> }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Detail { pub id: String, pub repo: String, pub license: String, pub note: Option<String>, pub files: Vec<FileRow>, pub variants: Vec<Variant> }

const BAY: &str = "https://huggingbay.xyz";

fn bytes_url(u: &str) -> bool {
    u.starts_with("https://huggingbay.xyz/api/downloads/")
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawFile {
    path: String,
    size_bytes: Option<u64>,
    sha256: Option<String>,
    #[serde(default)]
    hosted: Option<bool>,
    /// A string, or false or null when withheld.
    #[serde(default)]
    download_url: serde_json::Value,
}

impl RawFile {
    fn row(self) -> Option<FileRow> {
        let url = self.download_url.as_str().filter(|u| bytes_url(u))?.to_string();
        let sha256 = self.sha256.filter(|s| s.len() == 64)?;
        (self.hosted != Some(false)).then_some(())?;
        Some(FileRow { path: self.path, size: self.size_bytes?, sha256, url })
    }
}

/// Keep rows that have GGUF files hosted for download.
pub fn distil(lines: impl BufRead) -> Result<Vec<Entry>, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        id: String,
        repo: String,
        #[serde(rename = "type")]
        kind: Option<String>,
        license: Option<String>,
        download_count: Option<u64>,
        #[serde(default)]
        files: Vec<RawFile>,
    }
    let mut out = Vec::new();
    for line in lines.lines() {
        let line = line.map_err(|e| format!("the catalog download broke off: {e}"))?;
        if line.is_empty() {
            continue;
        }
        // Most rows have no GGUF; skipping them before parsing saves most of the work.
        if !line.contains(".gguf") && line.starts_with('{') {
            continue;
        }
        let r: Row = serde_json::from_str(&line).map_err(|e| format!("the catalog is not in the expected form: {e}"))?;
        let files: Vec<_> = r.files.into_iter().filter_map(RawFile::row).collect();
        let variants = variants(&files);
        if !variants.is_empty() {
            out.push(Entry { id: r.id, repo: r.repo, kind: r.kind.unwrap_or_default(), license: r.license.unwrap_or_default(), downloads: r.download_count.unwrap_or(0), variants });
        }
    }
    Ok(out)
}

/// `(label, bits per weight)` from a file name, last match winning.
fn quant(stem: &str) -> (String, f64) {
    let up = stem.to_uppercase();
    for part in up.rsplit(['-', '.', ' ', '/']) {
        // "FP16" and "F16" are the same thing.
        let label = part.strip_prefix("FP").map_or(part.to_string(), |r| format!("F{r}"));
        let body = label.trim_start_matches("UD_");
        let bits = match body {
            "F32" => 32.0,
            "F16" | "BF16" => 16.0,
            "MXFP4" => 4.4,
            b => {
                let q = b.strip_prefix("IQ").or_else(|| b.strip_prefix("TQ")).or_else(|| b.strip_prefix('Q'));
                let Some(n) = q.and_then(|q| q.chars().next()).and_then(|c| c.to_digit(10)) else { continue };
                if !b.contains('_') && b.len() > 3 {
                    continue;
                }
                n as f64 + if b.contains("_K_M") || b.contains("_K_L") { 0.9 } else if b.contains("_1") || b.contains("_K") { 0.6 } else { 0.5 }
            }
        };
        return (body.to_string(), bits);
    }
    ("GGUF".into(), 0.0)
}

/// `x-00001-of-00003` gives `(x, 1, 3)`.
fn shard(stem: &str) -> Option<(&str, u32, u32)> {
    let (head, total) = stem.rsplit_once("-of-")?;
    let (base, n) = head.rsplit_once('-')?;
    Some((base, n.parse().ok()?, total.parse().ok()?))
}

/// The downloadable model choices among some files, smallest first.
pub fn variants(files: &[FileRow]) -> Vec<Variant> {
    let mut groups: BTreeMap<String, (u32, Vec<(u32, FileRow)>)> = BTreeMap::new();
    for f in files {
        let lower = f.path.to_lowercase();
        let name = lower.rsplit('/').next().unwrap_or(&lower);
        if !name.ends_with(".gguf") || ["mmproj", "imatrix", "mtp-", "-mtp", "draft"].iter().any(|w| name.contains(w)) {
            continue;
        }
        let stem = &f.path[..f.path.len() - 5];
        let (base, n, total) = shard(stem).unwrap_or((stem, 1, 1));
        let g = groups.entry(base.to_string()).or_insert((total, vec![]));
        g.1.push((n, f.clone()));
    }
    let mut out: Vec<Variant> = groups
        .into_iter()
        .filter(|(_, (total, parts))| parts.len() == *total as usize)
        .map(|(base, (_, mut parts))| {
            parts.sort_by_key(|p| p.0);
            let (label, bits) = quant(base.rsplit('/').next().unwrap_or(&base));
            Variant { label, bits, size: parts.iter().map(|p| p.1.size).sum(), files: parts.into_iter().map(|p| p.1).collect() }
        })
        .collect();
    out.sort_by(|a, b| a.bits.total_cmp(&b.bits).then(a.size.cmp(&b.size)));
    out
}

pub fn parse_search(body: &str) -> Result<Page, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Row {
        id: String,
        repo: String,
        #[serde(rename = "type")]
        kind: Option<String>,
        license: Option<String>,
        download_count: Option<u64>,
        formats: Option<Vec<String>>,
        mirrored_bytes: Option<u64>,
        size_bytes: Option<u64>,
        download_ready: Option<bool>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        rows: Vec<Row>,
        total: Option<u64>,
        next_cursor: Option<serde_json::Value>,
    }
    let r: Raw = serde_json::from_str(body).map_err(|e| format!("unexpected search answer: {e}"))?;
    let hits: Vec<_> = r
        .rows
        .into_iter()
        .map(|r| Hit {
            id: r.id,
            repo: r.repo,
            kind: r.kind.unwrap_or_default(),
            license: r.license.unwrap_or_default(),
            downloads: r.download_count.unwrap_or(0),
            formats: r.formats.unwrap_or_default(),
            size: r.mirrored_bytes.or(r.size_bytes).filter(|s| *s > 0),
            ready: r.download_ready,
        })
        .collect();
    let next = r.next_cursor.and_then(|c| match c {
        serde_json::Value::String(s) => Some(s),
        serde_json::Value::Number(n) => Some(n.to_string()),
        _ => None,
    });
    Ok(Page { total: r.total.unwrap_or(hits.len() as u64).max(hits.len() as u64), hits, next })
}

pub fn parse_plan(body: &str) -> Result<Detail, String> {
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Artifact {
        repo: Option<String>,
        license_id: Option<String>,
    }
    #[derive(Deserialize)]
    #[serde(rename_all = "camelCase")]
    struct Raw {
        artifact_id: String,
        artifact: Artifact,
        files: Vec<RawFile>,
        reason: Option<String>,
    }
    let r: Raw = serde_json::from_str(body).map_err(|e| format!("unexpected file list: {e}"))?;
    let files: Vec<_> = r.files.into_iter().filter_map(RawFile::row).collect();
    Ok(Detail {
        id: r.artifact_id,
        repo: r.artifact.repo.ok_or("the file list names no repository")?,
        license: r.artifact.license_id.unwrap_or_default(),
        note: r.reason.filter(|s| !s.is_empty()),
        variants: variants(&files),
        files,
    })
}

pub fn search(query: &str, cursor: Option<&str>) -> Result<Page, String> {
    let cursor = cursor.map_or(String::new(), |c| format!("&cursor={}", encode(c)));
    parse_search(&http::text(&format!("{BAY}/api/v1/search?q={}&limit=30{cursor}", encode(query)))?)
}

pub fn detail(id: &str) -> Result<Detail, String> {
    parse_plan(&http::text(&format!("{BAY}/api/v1/artifacts/{}/download-plan", encode(id)))?)
}

/// A model's shape from the first bytes of its file.
pub fn header(file: &FileRow) -> Result<Shape, String> {
    // The architecture comes before the vocabulary, so this much is enough to size a model.
    const FIRST: u64 = 96 << 10;
    let body = http::open(&file.url, 0, Some(FIRST - 1))?;
    let mut v = Vec::new();
    body.reader.take(FIRST).read_to_end(&mut v).map_err(|e| format!("the connection dropped: {e}"))?;
    read_header(&v)
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Index {
    pub date: String,
    pub total: u64,
    pub entries: Vec<Entry>,
}

#[derive(Deserialize)]
struct SnapshotFile {
    url: String,
    bytes: u64,
    sha256: String,
}

#[derive(Deserialize)]
struct SnapshotInfo {
    date: String,
    rows: u64,
    files: BTreeMap<String, SnapshotFile>,
}

/// Counts and hashes what passes through.
struct Meter<R, F> {
    inner: R,
    hash: Sha256,
    seen: u64,
    report: F,
}

impl<R: Read, F: FnMut(u64)> Read for Meter<R, F> {
    fn read(&mut self, buf: &mut [u8]) -> std::io::Result<usize> {
        let n = self.inner.read(buf)?;
        self.hash.update(&buf[..n]);
        self.seen += n as u64;
        (self.report)(self.seen);
        Ok(n)
    }
}

/// The catalog of hosted GGUF models, refreshed when the site has published a newer one.
pub fn sync(dir: &Path, progress: &mut dyn FnMut(f64)) -> Result<Index, String> {
    let path = dir.join("catalog-2.json");
    let have: Option<Index> = crate::store::load(&path);
    let info: SnapshotInfo = match http::json(&format!("{BAY}/api/v1/catalog/snapshot")) {
        Ok(i) => i,
        // Offline: what we have is still useful.
        Err(e) => return have.ok_or(e),
    };
    if let Some(h) = have.as_ref().filter(|h| h.date == info.date) {
        progress(1.0);
        return Ok(h.clone());
    }
    let file = info.files.get("jsonl").ok_or("the catalog has no JSONL snapshot")?;
    match fetch_snapshot(file, progress) {
        Ok(entries) => {
            let index = Index { date: info.date, total: info.rows, entries };
            crate::store::save(&path, &index)?;
            Ok(index)
        }
        Err(e) => have.ok_or(e),
    }
}

fn fetch_snapshot(file: &SnapshotFile, progress: &mut dyn FnMut(f64)) -> Result<Vec<Entry>, String> {
    if !file.url.starts_with(BAY) {
        return Err("the catalog points somewhere unexpected".into());
    }
    let body = http::open(&file.url, 0, None)?;
    let total = file.bytes.max(1) as f64;
    let mut meter = Meter { inner: body.reader, hash: Sha256::new(), seen: 0, report: |n: u64| progress((n as f64 / total).min(1.0)) };
    let entries = distil(BufReader::with_capacity(1 << 18, GzDecoder::new(BufReader::with_capacity(1 << 16, &mut meter))))?;
    std::io::copy(&mut meter, &mut std::io::sink()).map_err(|e| e.to_string())?;
    if hex(&meter.hash.finalize()) != file.sha256 {
        return Err("the catalog download does not match its checksum".into());
    }
    Ok(entries)
}

pub fn hex(b: &[u8]) -> String {
    b.iter().map(|b| format!("{b:02x}")).collect()
}
