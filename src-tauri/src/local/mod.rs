//! Identifying cards on the device with a vision language model instead of the Ximilar API: the model catalog,
//! downloads with progress, loading the model once, and asking it about a card.

pub mod catalog;
pub mod compare;
pub mod download;
pub mod grade;
pub mod identify;
#[cfg(feature = "local-model")]
mod llama;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};
use tokio_util::sync::CancellationToken;

use crate::error::{Error, Result};
use crate::imaging;
use crate::model::GradingMode;
use crate::ximilar::recognition::Recognition;
use catalog::ModelSpec;

/// Long edges of the images given to the model. Over 512 px the model tiles the image (more detail, slower);
/// the strip is enlarged so the collector number is readable.
const CARD_LONG_EDGE: u32 = 768;
const STRIP_LONG_EDGE: u32 = 1024;
const MAX_ANSWER_TOKENS: u32 = 160;

/// Something that answers a prompt about an image (llama.cpp in the app, a fake in tests).
pub trait VisionModel: Send + Sync {
    fn generate(
        &self,
        image: &[u8],
        prompt: &str,
        grammar: Option<&str>,
        max_tokens: u32,
    ) -> Result<String>;
}

type Notify = Arc<dyn Fn(ModelStatus) + Send + Sync>;
type Loader = Arc<dyn Fn(&ModelSpec, &Path, &Path) -> Result<Arc<dyn VisionModel>> + Send + Sync>;

#[derive(Debug, Clone, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum ModelState {
    Missing,
    Downloading,
    Ready,
}

/// A model as the Settings page shows it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelStatus {
    pub id: String,
    pub name: String,
    pub tier: String,
    pub params: String,
    pub description: String,
    /// Memory the loaded model needs, roughly (MB).
    pub memory_mb: u32,
    pub license_name: String,
    pub license_url: String,
    pub homepage: String,
    pub size: u64,
    pub downloaded: u64,
    pub state: ModelState,
    /// The last download error.
    pub error: Option<String>,
    /// This build can run models (the `local-model` feature).
    pub supported: bool,
}

struct Download {
    cancel: CancellationToken,
    /// Bytes per file, model then projector.
    done: [u64; 2],
}

pub struct LocalModels {
    dir: PathBuf,
    http: reqwest::Client,
    downloads: Mutex<HashMap<String, Download>>,
    errors: Mutex<HashMap<String, String>>,
    loaded: tokio::sync::Mutex<Option<(String, Arc<dyn VisionModel>)>>,
    loader: RwLock<Loader>,
    /// Running comparisons by the id the UI gave them, to cancel them.
    pub comparing: Mutex<HashMap<String, CancellationToken>>,
    notify: RwLock<Option<Notify>>,
}

pub fn supported() -> bool {
    cfg!(feature = "local-model")
}

fn default_loader() -> Loader {
    Arc::new(|_spec, _model, _mmproj| {
        #[cfg(feature = "local-model")]
        {
            Ok(Arc::new(llama::LlamaVision::load(_model, _mmproj)?) as Arc<dyn VisionModel>)
        }
        #[cfg(not(feature = "local-model"))]
        {
            Err(Error::msg(
                "this build of cardloader can't run local models",
            ))
        }
    })
}

impl LocalModels {
    pub fn new(dir: PathBuf) -> Self {
        LocalModels {
            dir,
            http: reqwest::Client::builder()
                .user_agent(concat!("cardloader/", env!("CARGO_PKG_VERSION")))
                .connect_timeout(Duration::from_secs(20))
                .build()
                .expect("http client"),
            downloads: Mutex::new(HashMap::new()),
            errors: Mutex::new(HashMap::new()),
            loaded: tokio::sync::Mutex::new(None),
            loader: RwLock::new(default_loader()),
            comparing: Mutex::new(HashMap::new()),
            notify: RwLock::new(None),
        }
    }

    /// Where status changes go (the UI's `model-updated` event).
    pub fn on_change(&self, f: impl Fn(ModelStatus) + Send + Sync + 'static) {
        *self.notify.write().unwrap() = Some(Arc::new(f));
    }

    #[cfg(test)]
    pub fn set_loader(&self, loader: Loader) {
        *self.loader.write().unwrap() = loader;
    }

    fn file_path(&self, spec: &ModelSpec, name: &str) -> PathBuf {
        self.dir.join(spec.id).join(name)
    }

    fn paths(&self, spec: &ModelSpec) -> (PathBuf, PathBuf) {
        (
            self.file_path(spec, spec.model.name),
            self.file_path(spec, spec.mmproj.name),
        )
    }

    pub fn is_ready(&self, id: &str) -> bool {
        catalog::find(id).is_some_and(|spec| {
            let (m, p) = self.paths(spec);
            m.is_file() && p.is_file()
        })
    }

    pub fn status(&self, spec: &ModelSpec) -> ModelStatus {
        let downloads = self.downloads.lock().unwrap();
        let (state, downloaded) = match downloads.get(spec.id) {
            Some(d) => (ModelState::Downloading, d.done.iter().sum()),
            None if self.is_ready(spec.id) => (ModelState::Ready, spec.size()),
            None => {
                let (m, p) = self.paths(spec);
                (
                    ModelState::Missing,
                    download::present(&m) + download::present(&p),
                )
            }
        };
        ModelStatus {
            id: spec.id.into(),
            name: spec.name.into(),
            tier: spec.tier.into(),
            params: spec.params.into(),
            description: spec.description.into(),
            memory_mb: spec.memory_mb,
            license_name: spec.license_name.into(),
            license_url: spec.license_url.into(),
            homepage: spec.homepage.into(),
            size: spec.size(),
            downloaded,
            state,
            error: self.errors.lock().unwrap().get(spec.id).cloned(),
            supported: supported(),
        }
    }

    pub fn list(&self) -> Vec<ModelStatus> {
        catalog::MODELS.iter().map(|m| self.status(m)).collect()
    }

    fn emit(&self, spec: &ModelSpec) {
        let notify = self.notify.read().unwrap().clone();
        if let Some(f) = notify {
            f(self.status(spec));
        }
    }

    /// Starts (or resumes) downloading a model in the background.
    pub fn download(self: &Arc<Self>, id: &str) -> Result<ModelStatus> {
        let spec = catalog::find(id).ok_or_else(|| Error::msg("unknown model"))?;
        {
            let mut downloads = self.downloads.lock().unwrap();
            if downloads.contains_key(id) || self.is_ready(id) {
                drop(downloads);
                return Ok(self.status(spec));
            }
            downloads.insert(
                id.into(),
                Download {
                    cancel: CancellationToken::new(),
                    done: [0, 0],
                },
            );
        }
        self.errors.lock().unwrap().remove(id);
        let cancel = self.downloads.lock().unwrap()[id].cancel.clone();
        let this = self.clone();
        tauri::async_runtime::spawn(async move {
            let result = this.fetch_all(spec, &cancel).await;
            this.downloads.lock().unwrap().remove(spec.id);
            match result {
                Ok(()) => tracing::info!("model {} downloaded", spec.id),
                Err(_) if cancel.is_cancelled() => {}
                Err(e) => {
                    tracing::warn!("model {} download failed: {e}", spec.id);
                    this.errors
                        .lock()
                        .unwrap()
                        .insert(spec.id.into(), e.to_string());
                }
            }
            this.emit(spec);
        });
        Ok(self.status(spec))
    }

    async fn fetch_all(
        self: &Arc<Self>,
        spec: &'static ModelSpec,
        cancel: &CancellationToken,
    ) -> Result<()> {
        let last = Mutex::new(Instant::now() - Duration::from_secs(1));
        for (i, file) in spec.files().into_iter().enumerate() {
            let dest = self.file_path(spec, file.name);
            let progress = |n: u64| {
                if let Some(d) = self.downloads.lock().unwrap().get_mut(spec.id) {
                    d.done[i] = n;
                }
                // At most a few updates a second.
                let mut last = last.lock().unwrap();
                if last.elapsed() >= Duration::from_millis(250) {
                    *last = Instant::now();
                    drop(last);
                    self.emit(spec);
                }
            };
            download::fetch(&self.http, file, file.url, &dest, &progress, cancel).await?;
        }
        Ok(())
    }

    pub fn cancel(&self, id: &str) {
        if let Some(d) = self.downloads.lock().unwrap().get(id) {
            d.cancel.cancel();
        }
    }

    /// Removes a model's files (and a partial download).
    pub async fn delete(&self, id: &str) -> Result<ModelStatus> {
        let spec = catalog::find(id).ok_or_else(|| Error::msg("unknown model"))?;
        self.cancel(id);
        let mut loaded = self.loaded.lock().await;
        if loaded.as_ref().is_some_and(|(m, _)| m == id) {
            *loaded = None;
        }
        drop(loaded);
        // A cancelled download lets go of its file within moments.
        for _ in 0..50 {
            if !self.downloads.lock().unwrap().contains_key(id) {
                break;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        let dir = self.dir.join(spec.id);
        if dir.exists() {
            tokio::fs::remove_dir_all(&dir).await?;
        }
        self.errors.lock().unwrap().remove(id);
        self.emit(spec);
        Ok(self.status(spec))
    }

    async fn engine(&self, id: &str) -> Result<Arc<dyn VisionModel>> {
        let spec =
            catalog::find(id).ok_or_else(|| Error::msg(format!("unknown local model {id}")))?;
        if !self.is_ready(id) {
            return Err(Error::msg(format!(
                "the local model {} isn't downloaded — download it in Settings",
                spec.name
            )));
        }
        let mut loaded = self.loaded.lock().await;
        if let Some((m, engine)) = loaded.as_ref() {
            if m == id {
                return Ok(engine.clone());
            }
        }
        let (model, mmproj) = self.paths(spec);
        let loader = self.loader.read().unwrap().clone();
        // Loading reads a few hundred MB: off the async threads.
        let engine = tokio::task::spawn_blocking(move || loader(spec, &model, &mmproj))
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        *loaded = Some((id.to_string(), engine.clone()));
        Ok(engine)
    }

    /// Identifies the card in `image` (a file) with model `id`. Returns the recognition and the raw record
    /// stored with the item.
    pub async fn identify(&self, id: &str, image: &Path) -> Result<(Recognition, Value)> {
        let engine = self.engine(id).await?;
        let path = image.to_path_buf();
        let (card_jpeg, strip_jpeg) =
            tokio::task::spawn_blocking(move || -> Result<(Vec<u8>, Vec<u8>)> {
                let img = imaging::open(&path)?;
                let card = imaging::jpeg_bytes(&imaging::fit_long_edge(&img, CARD_LONG_EDGE), 90)?;
                let top = (img.height() as f64 * (1.0 - identify::STRIP_FRACTION)) as u32;
                let strip = img.crop_imm(0, top, img.width(), img.height() - top);
                let strip =
                    imaging::jpeg_bytes(&imaging::fit_long_edge(&strip, STRIP_LONG_EDGE), 92)?;
                Ok((card, strip))
            })
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        let started = Instant::now();
        let (card_answer, strip_answer) =
            tokio::task::spawn_blocking(move || -> Result<(String, String)> {
                let card = engine.generate(
                    &card_jpeg,
                    identify::CARD_PROMPT,
                    Some(&identify::card_grammar()),
                    MAX_ANSWER_TOKENS,
                )?;
                let strip = engine.generate(
                    &strip_jpeg,
                    identify::STRIP_PROMPT,
                    Some(&identify::strip_grammar()),
                    MAX_ANSWER_TOKENS,
                )?;
                Ok((card, strip))
            })
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        let seconds = started.elapsed().as_secs_f64();
        let (recognition, parsed) =
            identify::parse_answers(&card_answer, Some(&strip_answer)).map_err(Error::Msg)?;
        let raw = json!({
            "endpoint": format!("local/{id}"),
            "model": id,
            "seconds": (seconds * 100.0).round() / 100.0,
            "answers": [card_answer, strip_answer],
            "response": parsed,
        });
        Ok((recognition, raw))
    }

    /// Grades the card in `sides` (front, then back if there is one) with model `id`: a grade and its
    /// condition, a condition on `scale`, or centering only. Returns the grading and the raw record to store.
    pub async fn grade(
        &self,
        id: &str,
        sides: &[PathBuf],
        mode: GradingMode,
        scale: &'static grade::Scale,
    ) -> Result<grade::LocalGrading> {
        let engine = self.engine(id).await?;
        let paths = sides.to_vec();
        let images = tokio::task::spawn_blocking(move || -> Result<Vec<Vec<u8>>> {
            paths
                .iter()
                .map(|p| {
                    let img = imaging::open(p)?;
                    imaging::jpeg_bytes(&imaging::fit_long_edge(&img, CARD_LONG_EDGE), 90)
                })
                .collect()
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
        let started = Instant::now();
        let answers = tokio::task::spawn_blocking(move || -> Result<Vec<String>> {
            let (prompt, grammar) = match mode {
                GradingMode::Condition => (
                    grade::condition_prompt(scale),
                    grade::condition_grammar(scale),
                ),
                _ => (grade::GRADE_PROMPT.to_string(), grade::grade_grammar()),
            };
            images
                .iter()
                .map(|jpeg| engine.generate(jpeg, &prompt, Some(&grammar), MAX_ANSWER_TOKENS))
                .collect()
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
        let seconds = started.elapsed().as_secs_f64();
        let mut graded = grade::parse(mode, scale, &answers).map_err(Error::Msg)?;
        graded.raw = json!({
            "endpoint": mode.as_str(),
            "engine": format!("local/{id}"),
            "seconds": (seconds * 100.0).round() / 100.0,
            "answers": answers,
            "response": graded.raw,
        });
        Ok(graded)
    }
}

#[cfg(test)]
pub mod testing {
    use super::*;

    /// A model that always gives the same answer.
    pub struct FakeVision(pub String);

    impl VisionModel for FakeVision {
        fn generate(&self, image: &[u8], prompt: &str, _: Option<&str>, _: u32) -> Result<String> {
            assert!(!image.is_empty() && !prompt.is_empty());
            Ok(self.0.clone())
        }
    }

    /// Pretends `spec`'s files are downloaded and makes `answer` the model's answer.
    pub fn install_fake(models: &LocalModels, answer: &str) {
        for spec in catalog::MODELS {
            for file in spec.files() {
                let path = models.file_path(spec, file.name);
                std::fs::create_dir_all(path.parent().unwrap()).unwrap();
                std::fs::write(path, b"fake").unwrap();
            }
        }
        let answer = answer.to_string();
        models.set_loader(Arc::new(move |_, _, _| {
            Ok(Arc::new(FakeVision(answer.clone())))
        }));
    }
}

/// Runs the real model on real card images. Opt-in, since it needs the downloaded model:
/// `CARDLOADER_MODELS=<dir with lfm2.5-vl-450m/> CARDLOADER_CARDS=<dir of images> cargo test --release -p cardloader
/// real_model -- --ignored --nocapture`
#[cfg(all(test, feature = "local-model"))]
mod real_model {
    use super::*;

    /// `CARDLOADER_MODEL`, or the default model.
    fn test_model() -> String {
        std::env::var("CARDLOADER_MODEL").unwrap_or_else(|_| catalog::DEFAULT_MODEL.into())
    }

    /// Downloads the real files into `CARDLOADER_DOWNLOAD_TO` with the app's downloader.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_download() {
        let dir =
            PathBuf::from(std::env::var("CARDLOADER_DOWNLOAD_TO").expect("CARDLOADER_DOWNLOAD_TO"));
        let local = Arc::new(LocalModels::new(dir));
        let updates = Arc::new(Mutex::new(0usize));
        let counter = updates.clone();
        local.on_change(move |_| *counter.lock().unwrap() += 1);
        let id = test_model();
        local.download(&id).unwrap();
        let started = Instant::now();
        loop {
            let status = local.status(catalog::find(&id).unwrap());
            if status.state == ModelState::Ready {
                break;
            }
            assert!(status.error.is_none(), "{:?}", status.error);
            assert!(
                started.elapsed() < Duration::from_secs(900),
                "download too slow"
            );
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        println!(
            "downloaded in {:.1}s with {} progress updates",
            started.elapsed().as_secs_f64(),
            updates.lock().unwrap()
        );
    }

    /// Grades each card of `CARDLOADER_CARDS` (one image per card) in all three modes, on the TCGplayer scale.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_model_grades_cards() {
        let models = std::env::var("CARDLOADER_MODELS").expect("CARDLOADER_MODELS");
        let cards = std::env::var("CARDLOADER_CARDS").expect("CARDLOADER_CARDS");
        let local = LocalModels::new(PathBuf::from(models));
        let id = test_model();
        assert!(local.is_ready(&id), "model files missing");
        let mut paths: Vec<_> = std::fs::read_dir(cards)
            .unwrap()
            .map(|e| e.unwrap().path())
            .filter(|p| imaging::is_supported(p))
            .collect();
        paths.sort();
        let scale = grade::scale("tcgplayer");
        for path in &paths {
            let side = [path.clone()];
            let g = local
                .grade(&id, &side, GradingMode::Grade, scale)
                .await
                .unwrap();
            let c = local
                .grade(&id, &side, GradingMode::Condition, scale)
                .await
                .unwrap();
            println!(
                "{:<36} grade {:>4} {:<18} {:>5.2}s | condition {:<18} {:>5.2}s | {}",
                path.file_name().unwrap().to_string_lossy(),
                g.grade.unwrap_or_default(),
                g.condition.unwrap_or_default(),
                g.raw["seconds"].as_f64().unwrap(),
                c.condition.unwrap_or_default(),
                c.raw["seconds"].as_f64().unwrap(),
                g.raw["answers"][0]
            );
        }
    }

    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_model_reads_cards() {
        let models = std::env::var("CARDLOADER_MODELS").expect("CARDLOADER_MODELS");
        let cards = std::env::var("CARDLOADER_CARDS").expect("CARDLOADER_CARDS");
        let local = LocalModels::new(PathBuf::from(models));
        let id = test_model();
        assert!(local.is_ready(&id), "model files missing");
        let mut paths: Vec<_> = std::fs::read_dir(cards)
            .unwrap()
            .map(|e| e.unwrap().path())
            .collect();
        paths.sort();
        for path in paths.iter().filter(|p| imaging::is_supported(p)) {
            let (r, raw) = local.identify(&id, path).await.unwrap();
            println!(
                "{:<28} {:>5.2}s  {:<6} {}  | {}",
                path.file_name().unwrap().to_string_lossy(),
                raw["seconds"].as_f64().unwrap(),
                r.id_type,
                r.card.full_name,
                raw["answers"]
            );
            println!(
                "EVAL {}",
                json!({"file": path.file_name().unwrap().to_string_lossy(), "seconds": raw["seconds"],
                       "name": r.card.name, "number": r.card.card_number, "game": r.card.subcategory})
            );
        }
    }
}
