//! What the apps talk to: named calls in, JSON out, events on the side.
use crate::bench::{self, Report};
use crate::conditions::Conditions;
use crate::catalog::{self, FileRow, Index, Variant};
use crate::engine::{Engine, Place};
use crate::hardware::disk_free;
use crate::items::{item, Items};
use crate::predict::{load_from_shape, load_from_size, predict, Load, Prediction};
use crate::quality::Ratings;
use crate::recommend::shortlist;
use crate::run::{measure, Message, Piece, Server};
use crate::{store, Emit};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use std::collections::HashMap;
use std::io::Read;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};

#[derive(Default, Serialize, Deserialize)]
struct Settings {
    models_dir: Option<PathBuf>,
}

#[derive(Default)]
struct State {
    report: Option<Report>,
    index: Option<Arc<Index>>,
    /// Header loads by file checksum.
    loads: HashMap<String, Load>,
    chat: Option<(String, Arc<Server>)>,
    testing: bool,
}

struct Inner {
    data: PathBuf,
    models: PathBuf,
    engine: Option<Engine>,
    emit: Emit,
    ratings: Ratings,
    state: Mutex<State>,
    /// Held while the catalog is being refreshed, so two screens do not fetch it twice.
    syncing: Mutex<()>,
    stop_bench: AtomicBool,
    stop_chat: AtomicBool,
    stop_measure: AtomicBool,
}

pub struct App {
    inner: Arc<Inner>,
    items: Items,
}

fn param<T: DeserializeOwned>(p: &Value, k: &str) -> Result<T, String> {
    serde_json::from_value(p[k].clone()).map_err(|_| format!("missing or wrong parameter: {k}"))
}

fn value(v: impl Serialize) -> Result<Value, String> {
    serde_json::to_value(v).map_err(|e| e.to_string())
}

/// `owner/Some-Model-GGUF` reads as `Some Model`.
pub fn pretty(repo: &str) -> String {
    let name = repo.rsplit('/').next().unwrap_or(repo);
    name.split(['-', '_']).filter(|w| !w.eq_ignore_ascii_case("gguf") && !w.is_empty()).collect::<Vec<_>>().join(" ")
}

impl Inner {
    fn lock(&self) -> MutexGuard<'_, State> {
        self.state.lock().unwrap_or_else(|e| e.into_inner())
    }

    fn engine(&self) -> Result<&Engine, String> {
        self.engine.as_ref().ok_or_else(|| "the engine is missing from this install. Reinstall RunIt".into())
    }

    /// A second user of the engine would spoil the test's timing.
    fn free(&self) -> Result<(), String> {
        if self.lock().testing {
            return Err("a test is running, wait for it to finish".into());
        }
        Ok(())
    }

    fn report(&self) -> Result<Report, String> {
        self.lock().report.clone().ok_or_else(|| "run the test first".into())
    }

    fn place(&self) -> Place {
        match self.lock().report.as_ref().and_then(|r| r.calibration.gpu.and(r.machine.gpu.clone())) {
            Some(g) => Place::Gpu(g.id),
            None => Place::Cpu,
        }
    }

    fn index(&self) -> Result<Arc<Index>, String> {
        let _one = self.syncing.lock().unwrap_or_else(|e| e.into_inner());
        if let Some(i) = self.lock().index.clone() {
            return Ok(i);
        }
        let mut last = -1.0;
        let i = Arc::new(catalog::sync(&self.data, &mut |p| {
            if p - last >= 0.01 || p >= 1.0 && last < 1.0 {
                last = p;
                (self.emit)(json!({"type": "catalog", "progress": p}));
            }
        })?);
        self.lock().index = Some(i.clone());
        Ok(i)
    }

    /// Loads from file headers, a few at a time, remembered for good.
    fn loads(&self, vs: &[&Variant]) -> Vec<Option<Load>> {
        let todo: Vec<&FileRow> = {
            let s = self.lock();
            vs.iter().map(|v| &v.files[0]).filter(|f| !s.loads.contains_key(&f.sha256)).collect()
        };
        if !todo.is_empty() {
            let queue = Mutex::new(todo.into_iter());
            std::thread::scope(|t| {
                for _ in 0..6 {
                    t.spawn(|| loop {
                        let Some(f) = queue.lock().unwrap_or_else(|e| e.into_inner()).next() else { return };
                        if let Ok(shape) = catalog::header(f) {
                            self.lock().loads.insert(f.sha256.clone(), load_from_shape(&shape, f.size));
                        }
                    });
                }
            });
            let _ = store::save(&self.data.join("headers.json"), &self.lock().loads);
        }
        let s = self.lock();
        vs.iter().map(|v| s.loads.get(&v.files[0].sha256).map(|l| whole(l, v))).collect()
    }

    fn predict(&self, v: &Variant, load: Option<&Load>) -> Option<Prediction> {
        let r = self.lock().report.clone()?;
        let rough = load_from_size(v.size);
        Some(predict(load.unwrap_or(&rough), &r.machine, &r.calibration))
    }
}

/// A load read from the first shard, scaled to the whole model.
fn whole(l: &Load, v: &Variant) -> Load {
    let f = v.size as f64 / v.files[0].size.max(1) as f64;
    Load { file_bytes: v.size, touched_bytes: l.touched_bytes * f, ..l.clone() }
}

/// What the library holds is judged by the newest test, from the files themselves.
fn repredict(items: &Items, r: &Report) {
    for item in items.list().into_iter().filter(|i| i.runnable()) {
        let Ok(path) = items.path(&item.key) else { continue };
        let mut head = Vec::new();
        // Enough for the whole header, vocabulary included.
        if std::fs::File::open(path).and_then(|f| f.take(32 << 20).read_to_end(&mut head)).is_err() {
            continue;
        }
        if let Ok(shape) = crate::gguf::read_header(&head) {
            items.set_prediction(&item.key, predict(&load_from_shape(&shape, item.total), &r.machine, &r.calibration));
        }
    }
}

impl App {
    /// `engine`: where llama.cpp is, or look beside the program.
    pub fn open(data: PathBuf, engine: Option<PathBuf>, emit: Emit) -> Result<App, String> {
        std::fs::create_dir_all(&data).map_err(|e| format!("could not open {}: {e}", data.display()))?;
        let settings: Settings = store::load(&data.join("settings.json")).unwrap_or_default();
        let models = settings.models_dir.unwrap_or_else(|| data.join("models"));
        // A folder on an unplugged drive should not stop the app from opening.
        let _ = std::fs::create_dir_all(&models);
        let inner = Arc::new(Inner {
            engine: match engine {
                Some(d) => Engine::new(d).ok(),
                None => Engine::locate().ok(),
            },
            emit: emit.clone(),
            ratings: Ratings::bundled(),
            state: Mutex::new(State { report: store::load(&data.join("report.json")), loads: store::load(&data.join("headers.json")).unwrap_or_default(), ..State::default() }),
            syncing: Mutex::new(()),
            stop_bench: AtomicBool::new(false),
            stop_chat: AtomicBool::new(false),
            stop_measure: AtomicBool::new(false),
            models: models.clone(),
            data: data.clone(),
        });
        let refresh = Arc::new(|id: &str| catalog::detail(id).map(|d| d.files));
        Ok(App { items: Items::open(data, models, emit, refresh), inner })
    }

    pub fn call(&self, method: &str, p: &Value) -> Result<Value, String> {
        let i = &self.inner;
        match method {
            "state" => {
                let s = i.lock();
                Ok(json!({
                    "version": env!("CARGO_PKG_VERSION"),
                    "report": s.report,
                    "testing": s.testing,
                    "engine": i.engine.is_some(),
                    "items": self.items.list(),
                    "models_dir": i.models,
                    "disk_free": disk_free(&i.models),
                    "ratings_date": i.ratings.date,
                    "ratings_source": i.ratings.source,
                }))
            }
            "creature" => value(crate::creature::creature(&param::<String>(p, "name")?)),
            "bench.start" => self.bench(),
            "bench.stop" => {
                i.stop_bench.store(true, Ordering::Relaxed);
                Ok(Value::Null)
            }
            "recommend" => {
                let r = i.report()?;
                let index = i.index()?;
                let list = shortlist(&index.entries, &i.ratings, &r.machine, &r.calibration, &|v| i.loads(v));
                let mut v = value(list)?;
                v["catalog_date"] = json!(index.date);
                v["catalog_total"] = json!(index.total);
                Ok(v)
            }
            "search" => self.search(&param::<String>(p, "q")?, p["cursor"].as_str()),
            "model" => self.model(&param::<String>(p, "id")?),
            "download" => self.download(&param::<String>(p, "id")?, &param::<Vec<String>>(p, "paths")?),
            "pause" => value(self.items.pause(&param::<String>(p, "key")?)?),
            "resume" => value(self.items.resume(&param::<String>(p, "key")?)?),
            "remove" => {
                let key: String = param(p, "key")?;
                self.close_chat(Some(&key));
                value(self.items.remove(&key)?)
            }
            "path" => value(self.items.path(&param::<String>(p, "key")?)?),
            "measure" => self.measure(param(p, "key")?),
            "chat.open" => self.open_chat(param(p, "key")?),
            "chat.send" => self.send(param(p, "messages")?),
            "chat.stop" => {
                i.stop_chat.store(true, Ordering::Relaxed);
                Ok(Value::Null)
            }
            "chat.close" => {
                self.close_chat(None);
                Ok(Value::Null)
            }
            "settings.set" => {
                let dir: PathBuf = param(p, "models_dir")?;
                if !dir.is_absolute() {
                    return Err("choose a full folder path".into());
                }
                std::fs::create_dir_all(&dir).map_err(|e| format!("could not use that folder: {e}"))?;
                store::save(&i.data.join("settings.json"), &Settings { models_dir: Some(dir) })?;
                Ok(Value::Null)
            }
            _ => Err(format!("unknown call: {method}")),
        }
    }

    fn bench(&self) -> Result<Value, String> {
        let i = self.inner.clone();
        i.free()?;
        i.engine()?;
        i.lock().testing = true;
        // Anything else using the graphics processor would spoil the timing.
        self.close_chat(None);
        i.stop_bench.store(false, Ordering::Relaxed);
        let items = self.items.clone();
        std::thread::spawn(move || {
            let result = i.engine().and_then(|e| bench::run(e, &i.data.join("bench"), &i.emit, &i.stop_bench));
            i.lock().testing = false;
            match result {
                Ok(r) => {
                    let _ = store::save(&i.data.join("report.json"), &r);
                    i.lock().report = Some(r.clone());
                    repredict(&items, &r);
                    (i.emit)(json!({"type": "bench", "kind": "done", "report": r}));
                }
                Err(e) if e == "stopped" => (i.emit)(json!({"type": "bench", "kind": "stopped"})),
                Err(e) => (i.emit)(json!({"type": "bench", "kind": "failed", "error": e})),
            }
        });
        Ok(Value::Null)
    }

    fn search(&self, q: &str, cursor: Option<&str>) -> Result<Value, String> {
        let i = &self.inner;
        let page = catalog::search(q, cursor)?;
        // Never wait for the catalog to show results; speeds appear once it is here.
        let index = i.lock().index.clone();
        let hits = page
            .hits
            .iter()
            .map(|h| {
                let mut v = value(h)?;
                let rating = i.ratings.find(&h.repo);
                v["name"] = json!(rating.map_or_else(|| pretty(&h.repo), |r| r.name.clone()));
                v["rating"] = value(rating)?;
                if let Some(e) = index.as_ref().and_then(|x| x.entries.iter().find(|e| e.id == h.id)).filter(|e| e.kind == "llm") {
                    let usual = e.variants.iter().find(|v| v.label == "Q4_K_M").unwrap_or(&e.variants[0]);
                    let load = i.lock().loads.get(&usual.files[0].sha256).map(|l| whole(l, usual));
                    v["label"] = json!(usual.label);
                    v["size"] = json!(usual.size);
                    v["prediction"] = value(i.predict(usual, load.as_ref()))?;
                }
                Ok(v)
            })
            .collect::<Result<Vec<_>, String>>()?;
        Ok(json!({"hits": hits, "total": page.total, "next": page.next}))
    }

    fn model(&self, id: &str) -> Result<Value, String> {
        let i = &self.inner;
        let d = catalog::detail(id)?;
        let kind = i.lock().index.as_ref().and_then(|x| x.entries.iter().find(|e| e.id == id).map(|e| e.kind.clone()));
        // Speed means tokens per second, which only a language model has.
        let speaks = kind.as_deref().unwrap_or("llm") == "llm";
        let firsts: Vec<&Variant> = d.variants.iter().take(1).collect();
        let load = if speaks { i.loads(&firsts).pop().flatten() } else { None };
        let variants = d
            .variants
            .iter()
            .map(|v| {
                let scaled = load.as_ref().map(|l| Load { file_bytes: v.size, touched_bytes: l.touched_bytes * v.size as f64 / firsts[0].size as f64, ..l.clone() });
                Ok(json!({"variant": v, "prediction": if speaks { value(i.predict(v, scaled.as_ref()))? } else { Value::Null }}))
            })
            .collect::<Result<Vec<_>, String>>()?;
        let in_variant = |f: &FileRow| d.variants.iter().any(|v| v.files.iter().any(|x| x.path == f.path));
        let capacity = i.lock().report.as_ref().map(|r| r.machine.capacity());
        Ok(json!({
            "id": d.id, "repo": d.repo, "name": i.ratings.find(&d.repo).map_or_else(|| pretty(&d.repo), |r| r.name.clone()), "license": d.license, "note": d.note,
            "rating": i.ratings.find(&d.repo), "kind": kind, "variants": variants,
            "others": d.files.iter().filter(|f| !in_variant(f)).collect::<Vec<_>>(),
            "capacity": capacity,
        }))
    }

    fn download(&self, id: &str, paths: &[String]) -> Result<Value, String> {
        let i = &self.inner;
        let d = catalog::detail(id)?;
        let files: Vec<FileRow> = paths
            .iter()
            .map(|p| d.files.iter().find(|f| &f.path == p).cloned().ok_or_else(|| format!("{p} is not available right now")))
            .collect::<Result<_, _>>()?;
        let variant = d.variants.iter().find(|v| v.files.iter().map(|f| &f.path).eq(paths.iter()));
        let (label, prediction) = match variant {
            Some(v) => {
                let load = i.loads(&[v]).pop().flatten();
                (v.label.clone(), i.predict(v, load.as_ref()))
            }
            None if files.len() == 1 => (files[0].path.clone(), None),
            None => (format!("{} files", files.len()), None),
        };
        let it = item(id, &d.repo, &i.ratings.find(&d.repo).map_or_else(|| pretty(&d.repo), |r| r.name.clone()), &label, files, prediction)?;
        let key = it.key.clone();
        self.items.add(it)?;
        Ok(json!({"key": key}))
    }

    fn measure(&self, key: String) -> Result<Value, String> {
        let i = self.inner.clone();
        let items = self.items.clone();
        i.free()?;
        let path = items.path(&key)?;
        i.engine()?;
        self.close_chat(None);
        i.stop_measure.store(false, Ordering::Relaxed);
        let place = i.place();
        let layers = items
            .list()
            .iter()
            .find(|x| x.key == key)
            .and_then(|x| Some((x.prediction.as_ref()?.gpu_share, i.lock().loads.get(&x.files[0].sha256)?.layers)))
            .filter(|(share, _)| *share < 1.0)
            .map_or(99, |(share, layers)| (share * layers as f64) as u32);
        std::thread::spawn(move || {
            let m = i.engine().and_then(|e| measure(e, &path, &place, layers, &i.stop_measure));
            (i.emit)(match m {
                Ok(m) => {
                    items.set_measured(&key, m);
                    json!({"type": "measure", "key": key, "kind": "done", "measured": m})
                }
                Err(e) => json!({"type": "measure", "key": key, "kind": "failed", "error": e}),
            });
        });
        Ok(Value::Null)
    }

    fn open_chat(&self, key: String) -> Result<Value, String> {
        let i = &self.inner;
        i.free()?;
        let path = self.items.path(&key)?;
        if i.lock().chat.as_ref().is_some_and(|(k, _)| *k == key) {
            return Ok(Value::Null);
        }
        self.close_chat(None);
        i.stop_chat.store(false, Ordering::Relaxed);
        let s = Arc::new(Server::start(i.engine()?, &path, &i.place(), &i.stop_chat)?);
        i.lock().chat = Some((key, s));
        Ok(Value::Null)
    }

    fn send(&self, messages: Vec<Message>) -> Result<Value, String> {
        let i = self.inner.clone();
        let Some((key, server)) = i.lock().chat.clone() else { return Err("open a model first".into()) };
        i.stop_chat.store(false, Ordering::Relaxed);
        std::thread::spawn(move || {
            let say = |v: Value| (i.emit)(v);
            let result = server.chat(
                &messages,
                &mut |p| match p {
                    Piece::Text(t) => say(json!({"type": "chat", "key": key, "kind": "text", "text": t})),
                    Piece::Thought(t) => say(json!({"type": "chat", "key": key, "kind": "thought", "text": t})),
                    Piece::Speed { tokens_per_s, prompt_tokens_per_s } => say(json!({"type": "chat", "key": key, "kind": "speed", "tokens_per_s": tokens_per_s, "prompt_tokens_per_s": prompt_tokens_per_s})),
                    Piece::End => {}
                },
                &i.stop_chat,
            );
            say(match result {
                Ok(()) => json!({"type": "chat", "key": key, "kind": "end", "text": Conditions::now().note()}),
                Err(e) => json!({"type": "chat", "key": key, "kind": "failed", "error": e}),
            });
        });
        Ok(Value::Null)
    }

    /// Unload the model, or only if it is this one.
    fn close_chat(&self, only: Option<&str>) {
        let mut s = self.inner.lock();
        if s.chat.as_ref().is_some_and(|(k, _)| only.is_none_or(|o| o == k)) {
            self.inner.stop_chat.store(true, Ordering::Relaxed);
            s.chat = None;
        }
    }

    #[doc(hidden)]
    pub fn pretend_testing(&self) {
        self.inner.lock().testing = true;
    }
}

impl Drop for App {
    fn drop(&mut self) {
        for s in [&self.inner.stop_bench, &self.inner.stop_chat, &self.inner.stop_measure] {
            s.store(true, Ordering::Relaxed);
        }
        self.items.close();
        self.close_chat(None);
    }
}
