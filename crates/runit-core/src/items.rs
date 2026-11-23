//! Downloads and the library: one list, saved across launches.
use crate::catalog::FileRow;
use crate::download::{fetch, part_path, safe_path};
use crate::hardware::disk_free;
use crate::predict::Prediction;
use crate::Emit;
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Status { Queued, Active, Paused, Failed, Ready }

#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Measured { pub tokens_per_s: f64, pub prompt_tokens_per_s: f64 }

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Item {
    pub key: String, pub id: String, pub repo: String, pub name: String, pub label: String,
    pub files: Vec<FileRow>, pub total: u64, pub done: u64, pub status: Status, pub error: Option<String>,
    pub prediction: Option<Prediction>, pub measured: Option<Measured>,
    #[serde(default)] pub bytes_per_s: f64,
}

/// Re-reads the live file list for an id, so a saved checksum or link is never stale.
pub type Refresh = Arc<dyn Fn(&str) -> Result<Vec<FileRow>, String> + Send + Sync>;

const AT_ONCE: usize = 2;
const SPARE_DISK: u64 = 1 << 30;

pub fn item(id: &str, repo: &str, name: &str, label: &str, files: Vec<FileRow>, prediction: Option<Prediction>) -> Result<Item, String> {
    let first = files.first().ok_or("nothing to download")?;
    for f in &files {
        safe_path(Path::new("m"), repo, &f.path)?;
    }
    Ok(Item {
        key: format!("{repo}/{}", first.path),
        id: id.into(),
        repo: repo.into(),
        name: name.into(),
        label: label.into(),
        total: files.iter().map(|f| f.size).sum(),
        files,
        done: 0,
        status: Status::Queued,
        error: None,
        prediction,
        measured: None,
        bytes_per_s: 0.0,
    })
}

#[derive(Default)]
struct State {
    items: Vec<Item>,
    stops: HashMap<String, Arc<AtomicBool>>,
}

struct Inner {
    data: PathBuf,
    models: PathBuf,
    emit: Emit,
    refresh: Refresh,
    state: Mutex<State>,
}

#[derive(Clone)]
pub struct Items(Arc<Inner>);

impl Items {
    pub fn open(data: PathBuf, models: PathBuf, emit: Emit, refresh: Refresh) -> Items {
        let mut items: Vec<Item> = crate::store::load(&data.join("items.json")).unwrap_or_default();
        for i in &mut items {
            i.bytes_per_s = 0.0;
            match i.status {
                Status::Active | Status::Queued => i.status = Status::Paused,
                Status::Ready if !i.files.iter().all(|f| safe_path(&models, &i.repo, &f.path).is_ok_and(|p| p.is_file())) => {
                    i.status = Status::Failed;
                    i.error = Some("the file is no longer on this disk".into());
                    i.done = 0;
                }
                _ => {}
            }
        }
        Items(Arc::new(Inner { data, models, emit, refresh, state: Mutex::new(State { items, stops: HashMap::new() }) }))
    }

    fn lock(&self) -> MutexGuard<'_, State> {
        self.0.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    pub fn list(&self) -> Vec<Item> {
        self.lock().items.clone()
    }

    fn get(&self, key: &str) -> Result<Item, String> {
        self.lock().items.iter().find(|i| i.key == key).cloned().ok_or_else(|| "that model is not in the library".into())
    }

    /// Change one item, save the list when asked, and announce it.
    fn update(&self, key: &str, save: bool, f: impl FnOnce(&mut Item)) -> bool {
        let mut s = self.lock();
        let Some(i) = s.items.iter_mut().find(|i| i.key == key) else { return false };
        f(i);
        let shown = i.clone();
        if save {
            let _ = crate::store::save(&self.0.data.join("items.json"), &s.items);
        }
        drop(s);
        (self.0.emit)(json!({"type": "item", "item": shown}));
        true
    }

    pub fn add(&self, item: Item) -> Result<(), String> {
        if let Ok(have) = self.get(&item.key) {
            return match have.status {
                Status::Paused | Status::Failed => self.resume(&item.key),
                _ => Err("this model is already in your library".into()),
            };
        }
        let busy: u64 = self.lock().items.iter().filter(|i| i.status != Status::Ready).map(|i| i.total - i.done.min(i.total)).sum();
        if let Some(free) = disk_free(&self.0.models) {
            if item.total.saturating_add(busy).saturating_add(SPARE_DISK) > free {
                let gb = |b: u64| b as f64 / 1e9;
                return Err(format!("not enough disk space: this needs {:.1} GB and {:.1} GB is free", gb(item.total), gb(free)));
            }
        }
        let key = item.key.clone();
        self.lock().items.insert(0, item);
        self.update(&key, true, |_| {});
        self.pump();
        Ok(())
    }

    pub fn pause(&self, key: &str) -> Result<(), String> {
        self.get(key)?;
        if let Some(s) = self.lock().stops.get(key) {
            s.store(true, Ordering::Relaxed);
        }
        self.update(key, true, |i| {
            if matches!(i.status, Status::Active | Status::Queued) {
                i.status = Status::Paused;
                i.bytes_per_s = 0.0;
            }
        });
        self.pump();
        Ok(())
    }

    pub fn resume(&self, key: &str) -> Result<(), String> {
        self.get(key)?;
        self.update(key, true, |i| {
            if matches!(i.status, Status::Paused | Status::Failed) {
                i.status = Status::Queued;
                i.error = None;
            }
        });
        self.pump();
        Ok(())
    }

    pub fn remove(&self, key: &str) -> Result<(), String> {
        let item = self.get(key)?;
        let running = {
            let mut s = self.lock();
            s.items.retain(|i| i.key != key);
            let _ = crate::store::save(&self.0.data.join("items.json"), &s.items);
            s.stops.get(key).cloned()
        };
        (self.0.emit)(json!({"type": "item", "removed": key}));
        match running {
            // The worker deletes the files once it has let go of them.
            Some(stop) => stop.store(true, Ordering::Relaxed),
            None => self.delete_files(&item)?,
        }
        self.pump();
        Ok(())
    }

    fn delete_files(&self, item: &Item) -> Result<(), String> {
        for f in &item.files {
            let p = safe_path(&self.0.models, &item.repo, &f.path)?;
            for p in [part_path(&p), p.clone()] {
                match std::fs::remove_file(&p) {
                    Err(e) if e.kind() != std::io::ErrorKind::NotFound => return Err(format!("could not delete {}: {e}", p.display())),
                    _ => {}
                }
            }
            let mut d = p.parent();
            while let Some(dir) = d.filter(|d| *d != self.0.models) {
                // Fails, harmlessly, at the first folder that still holds something.
                if std::fs::remove_dir(dir).is_err() {
                    break;
                }
                d = dir.parent();
            }
        }
        Ok(())
    }

    /// The file to load. llama.cpp finds the other shards from the first.
    pub fn path(&self, key: &str) -> Result<PathBuf, String> {
        let i = self.get(key)?;
        if i.status != Status::Ready {
            return Err("that model has not finished downloading".into());
        }
        safe_path(&self.0.models, &i.repo, &i.files[0].path)
    }

    pub fn set_measured(&self, key: &str, m: Measured) {
        self.update(key, true, |i| i.measured = Some(m));
    }

    /// Stop every worker. What was fetched stays for next time.
    pub fn close(&self) {
        for s in self.lock().stops.values() {
            s.store(true, Ordering::Relaxed);
        }
    }

    fn pump(&self) {
        let next = {
            let mut s = self.lock();
            if s.stops.len() >= AT_ONCE {
                return;
            }
            let Some(key) = s.items.iter().rev().find(|i| i.status == Status::Queued && !s.stops.contains_key(&i.key)).map(|i| i.key.clone()) else { return };
            let stop = Arc::new(AtomicBool::new(false));
            s.stops.insert(key.clone(), stop.clone());
            (key, stop)
        };
        let (key, stop) = next;
        self.update(&key, true, |i| i.status = Status::Active);
        let me = self.clone();
        std::thread::spawn(move || {
            let item = me.get(&key).ok();
            let result = me.work(&key, &stop);
            me.lock().stops.remove(&key);
            let known = match result {
                Ok(()) => me.update(&key, true, |i| {
                    i.status = Status::Ready;
                    i.done = i.total;
                    i.bytes_per_s = 0.0;
                }),
                Err(_) if stop.load(Ordering::Relaxed) => me.update(&key, true, |i| {
                    i.bytes_per_s = 0.0;
                    // A resume may already have queued it again.
                    if i.status == Status::Active {
                        i.status = Status::Paused;
                    }
                }),
                Err(e) => me.update(&key, true, |i| {
                    i.status = Status::Failed;
                    i.error = Some(e);
                    i.bytes_per_s = 0.0;
                }),
            };
            if !known {
                if let Some(gone) = item {
                    let _ = me.delete_files(&gone);
                }
            }
            me.pump();
        });
        self.pump();
    }

    fn work(&self, key: &str, stop: &AtomicBool) -> Result<(), String> {
        let mut item = self.get(key)?;
        if let Ok(live) = (self.0.refresh)(&item.id) {
            for f in &mut item.files {
                if let Some(l) = live.iter().find(|l| l.path == f.path) {
                    *f = l.clone();
                }
            }
            let files = item.files.clone();
            self.update(key, true, |i| {
                i.total = files.iter().map(|f| f.size).sum();
                i.files = files;
            });
        }
        let mut before = 0;
        for f in &item.files {
            let dest = safe_path(&self.0.models, &item.repo, &f.path)?;
            if dest.metadata().is_ok_and(|m| m.len() == f.size) {
                before += f.size;
                continue;
            }
            let (mut shown, mut mark) = (Instant::now(), None::<(Instant, u64)>);
            let mut speed = 0.0;
            let mut report = |n: u64| {
                let (t0, n0) = *mark.get_or_insert((Instant::now(), n));
                let dt = t0.elapsed().as_secs_f64();
                if dt >= 1.0 {
                    let now = (n - n0.min(n)) as f64 / dt;
                    speed = if speed == 0.0 { now } else { speed * 0.7 + now * 0.3 };
                    mark = Some((Instant::now(), n));
                }
                if shown.elapsed().as_millis() >= 100 || n == f.size {
                    shown = Instant::now();
                    self.update(key, false, |i| {
                        i.done = before + n;
                        i.bytes_per_s = speed;
                    });
                }
            };
            fetch(&f.url, &dest, f.size, &f.sha256, &mut report, stop)?;
            before += f.size;
        }
        Ok(())
    }
}
