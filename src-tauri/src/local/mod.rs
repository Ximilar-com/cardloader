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
use crate::model::{GradingMode, IdMode};
use crate::ximilar::recognition::Recognition;
use catalog::ModelSpec;

/// Long edges of the images given to the model. Over 512 px the model tiles the image (more detail, slower);
/// the strip is enlarged so the collector number is readable.
const CARD_LONG_EDGE: u32 = 768;
const STRIP_LONG_EDGE: u32 = 1024;
const MAX_ANSWER_TOKENS: u32 = 160;
/// Every field at once, the set name included.
const MAX_WHOLE_TOKENS: u32 = 240;
/// JPEGs of one card: the front, its back if there is one, and the part of the front with the number.
type Images = (Vec<u8>, Option<Vec<u8>>, Vec<u8>);

/// Something that answers a prompt about images (llama.cpp in the app, a fake in tests): one image, or several of
/// the same card (its front and back), which the prompt names in order.
pub trait VisionModel: Send + Sync {
    fn generate(
        &self,
        images: &[&[u8]],
        prompt: &str,
        grammar: Option<&str>,
        max_tokens: u32,
    ) -> Result<String>;
}

type Notify = Arc<dyn Fn(ModelStatus) + Send + Sync>;
type Loader = Arc<dyn Fn(&ModelSpec, &Path, &Path, &Device) -> Result<Loaded> + Send + Sync>;

/// Where models run (`settings.local_device`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Device {
    /// The best GPU there is (a discrete one first), else the CPU.
    Auto,
    Cpu,
    /// A GPU by the name llama.cpp gives it ("Vulkan1", "CUDA0").
    Named(String),
}

impl Device {
    /// "" is automatic, "cpu" the CPU, anything else a GPU's name.
    pub fn from_setting(s: &str) -> Device {
        match s {
            "" => Device::Auto,
            "cpu" => Device::Cpu,
            name => Device::Named(name.into()),
        }
    }
}

/// A loaded model and where it runs.
pub struct Loaded {
    pub engine: Arc<dyn VisionModel>,
    /// The device, as people know it: "NVIDIA GeForce RTX 4070", "Apple M2 Pro", "CPU".
    pub runs_on: String,
    /// Layers on the GPU, of `layers`: fewer when the GPU's free memory couldn't hold them all.
    pub gpu_layers: u32,
    pub layers: u32,
}

/// A device llama.cpp can run models on.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComputeDevice {
    /// llama.cpp's name for it ("Vulkan0", "CUDA0", "MTL0", "CPU"): what `settings.local_device` stores.
    pub name: String,
    /// What it is ("NVIDIA GeForce RTX 4070").
    pub description: String,
    /// "gpu", "igpu" (integrated), "cpu", "accel".
    pub kind: String,
    /// "Vulkan", "CUDA", "MTL" (Metal), "CPU".
    pub backend: String,
    pub memory_total: u64,
    pub memory_free: u64,
}

/// The model in memory and where it ended up.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ActiveModel {
    pub model: String,
    pub runs_on: String,
    pub gpu_layers: u32,
    pub layers: u32,
    /// Why it runs on the CPU though a GPU was wanted (loading there failed).
    pub fallback: Option<String>,
}

/// What Settings shows about running models: the devices and the loaded model.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DeviceList {
    /// This build can run models (the `local-model` feature).
    pub supported: bool,
    pub devices: Vec<ComputeDevice>,
    pub active: Option<ActiveModel>,
}

struct LoadedModel {
    id: String,
    device: Device,
    engine: Arc<dyn VisionModel>,
}

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
    pub maker: String,
    pub license_name: String,
    pub license_url: String,
    /// The license limits commercial use.
    pub commercial_limit: bool,
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
    loaded: tokio::sync::Mutex<Option<LoadedModel>>,
    /// Where the loaded model runs; apart from `loaded`, which is held while a model loads.
    active: Mutex<Option<ActiveModel>>,
    device: RwLock<Device>,
    loader: RwLock<Loader>,
    /// Running comparisons by the id the UI gave them, to cancel them.
    pub comparing: Mutex<HashMap<String, CancellationToken>>,
    notify: RwLock<Option<Notify>>,
}

pub fn supported() -> bool {
    cfg!(feature = "local-model")
}

/// The devices models can run on (none in a build without local models).
pub fn devices() -> Vec<ComputeDevice> {
    #[cfg(feature = "local-model")]
    {
        llama::devices()
    }
    #[cfg(not(feature = "local-model"))]
    {
        Vec::new()
    }
}

fn default_loader() -> Loader {
    Arc::new(|_spec, _model, _mmproj, _device| {
        #[cfg(feature = "local-model")]
        {
            llama::LlamaVision::load(_spec, _model, _mmproj, _device)
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
            active: Mutex::new(None),
            device: RwLock::new(Device::Auto),
            loader: RwLock::new(default_loader()),
            comparing: Mutex::new(HashMap::new()),
            notify: RwLock::new(None),
        }
    }

    /// Where models run from now on; a model loaded elsewhere is loaded again when next used.
    pub fn set_device(&self, device: Device) {
        *self.device.write().unwrap() = device;
    }

    /// The devices there are and where the loaded model runs.
    pub fn device_list(&self) -> DeviceList {
        DeviceList {
            supported: supported(),
            devices: devices(),
            active: self.active.lock().unwrap().clone(),
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
            maker: spec.maker.into(),
            license_name: spec.license_name.into(),
            license_url: spec.license_url.into(),
            commercial_limit: spec.commercial_limit,
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
        if loaded.as_ref().is_some_and(|l| l.id == id) {
            *loaded = None;
            *self.active.lock().unwrap() = None;
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
        let device = self.device.read().unwrap().clone();
        let mut loaded = self.loaded.lock().await;
        if let Some(l) = loaded.as_ref() {
            if l.id == id && l.device == device {
                return Ok(l.engine.clone());
            }
        }
        // Let go of the last model first: both may not fit in memory.
        *loaded = None;
        *self.active.lock().unwrap() = None;
        let (model, mmproj) = self.paths(spec);
        let loader = self.loader.read().unwrap().clone();
        let wanted = device.clone();
        // Loading reads gigabytes: off the async threads. A GPU that fails (its driver, its memory) leaves the CPU.
        let (l, fallback) =
            tokio::task::spawn_blocking(move || match loader(spec, &model, &mmproj, &wanted) {
                Ok(l) => Ok((l, None)),
                Err(e) if wanted != Device::Cpu => {
                    tracing::warn!(
                        "model {} failed on {wanted:?}, trying the CPU: {e}",
                        spec.id
                    );
                    loader(spec, &model, &mmproj, &Device::Cpu).map(|l| (l, Some(e.to_string())))
                }
                Err(e) => Err(e),
            })
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        *self.active.lock().unwrap() = Some(ActiveModel {
            model: id.to_string(),
            runs_on: l.runs_on,
            gpu_layers: l.gpu_layers,
            layers: l.layers,
            fallback,
        });
        *loaded = Some(LoadedModel {
            id: id.to_string(),
            device,
            engine: l.engine.clone(),
        });
        Ok(l.engine)
    }

    /// Identifies the card in `front` (a file) with model `id`; `kind` is what the session holds (its "Identify
    /// as"). `back`, the card's back when the session reads both sides, is sent to models that read the whole card;
    /// small models read it for a sports card (number, year and set are printed there), else the front's bottom.
    /// A comic (`IdMode::Comics`) is asked about as a cover (`identify::COMIC_PROMPT`), never with its back.
    /// Returns the recognition and the raw record stored with the item.
    pub async fn identify(
        &self,
        id: &str,
        front: &Path,
        back: Option<&Path>,
        kind: IdMode,
    ) -> Result<(Recognition, Value)> {
        let spec =
            catalog::find(id).ok_or_else(|| Error::msg(format!("unknown local model {id}")))?;
        let engine = self.engine(id).await?;
        let whole = spec.family.reads_whole_card();
        // A comic is read off its cover: the back is an advert.
        let comic = kind == IdMode::Comics;
        let front = front.to_path_buf();
        let back = back.filter(|_| !comic).map(Path::to_path_buf);
        // The whole card, its back, and for small models the enlarged part of the front with the number: the bottom
        // strip of a card, the top of a comic's cover.
        let (card, back, part) = tokio::task::spawn_blocking(move || -> Result<Images> {
            let img = imaging::open(&front)?;
            let card = imaging::jpeg_bytes(&imaging::fit_long_edge(&img, CARD_LONG_EDGE), 90)?;
            let back = back
                .map(|b| {
                    let b = imaging::open(&b)?;
                    imaging::jpeg_bytes(&imaging::fit_long_edge(&b, CARD_LONG_EDGE), 90)
                })
                .transpose()?;
            let part = if comic {
                let height = (img.height() as f64 * identify::COVER_TOP_FRACTION) as u32;
                img.crop_imm(0, 0, img.width(), height.max(1))
            } else {
                let top = (img.height() as f64 * (1.0 - identify::STRIP_FRACTION)) as u32;
                img.crop_imm(0, top, img.width(), img.height() - top)
            };
            let part = imaging::jpeg_bytes(&imaging::fit_long_edge(&part, STRIP_LONG_EDGE), 92)?;
            Ok((card, back, part))
        })
        .await
        .map_err(|e| Error::msg(e.to_string()))??;
        let started = Instant::now();
        let (answers, sent) =
            tokio::task::spawn_blocking(move || -> Result<(Vec<String>, &str)> {
                if comic && whole {
                    let answer = engine.generate(
                        &[&card],
                        identify::COMIC_PROMPT,
                        Some(&identify::comic_grammar()),
                        MAX_WHOLE_TOKENS,
                    )?;
                    return Ok((vec![answer], "front"));
                }
                if comic {
                    let first = engine.generate(
                        &[&card],
                        identify::COVER_PROMPT,
                        Some(&identify::cover_grammar()),
                        MAX_ANSWER_TOKENS,
                    )?;
                    let second = engine.generate(
                        &[&part],
                        identify::COVER_TOP_PROMPT,
                        Some(&identify::cover_top_grammar()),
                        MAX_ANSWER_TOKENS,
                    )?;
                    return Ok((vec![first, second], "front+top"));
                }
                if whole {
                    // Both sides in one question: measured on sports cards, faster than asking about the front first.
                    let mut all: Vec<&[u8]> = vec![&card];
                    all.extend(back.as_deref());
                    let answer = engine.generate(
                        &all,
                        &identify::whole_prompt(all.len() > 1),
                        Some(&identify::whole_grammar(kind)),
                        MAX_WHOLE_TOKENS,
                    )?;
                    return Ok((
                        vec![answer],
                        if all.len() > 1 { "front+back" } else { "front" },
                    ));
                }
                let first = engine.generate(
                    &[&card],
                    identify::CARD_PROMPT,
                    Some(&identify::card_grammar(kind)),
                    MAX_ANSWER_TOKENS,
                )?;
                // A sports card's number is on its back; a trading card's at the bottom of its front.
                let (image, prompt, sent) = match back.as_deref() {
                    Some(back) if kind == IdMode::Sport || identify::is_sport(&first) => {
                        (back, identify::BACK_PROMPT, "front+back")
                    }
                    _ => (part.as_slice(), identify::STRIP_PROMPT, "front+strip"),
                };
                let second = engine.generate(
                    &[image],
                    prompt,
                    Some(&identify::strip_grammar()),
                    MAX_ANSWER_TOKENS,
                )?;
                Ok((vec![first, second], sent))
            })
            .await
            .map_err(|e| Error::msg(e.to_string()))??;
        let seconds = started.elapsed().as_secs_f64();
        let (recognition, parsed) = if comic {
            identify::parse_comic(&answers[0], answers.get(1).map(String::as_str))
        } else if whole {
            identify::parse_whole(&answers[0])
        } else {
            identify::parse_answers(&answers[0], answers.get(1).map(String::as_str))
        }
        .map_err(Error::Msg)?;
        let raw = json!({
            "endpoint": format!("local/{id}"),
            "model": id,
            "seconds": (seconds * 100.0).round() / 100.0,
            // What the model saw: the card, then its back, its enlarged bottom strip or the top of a comic's cover.
            "images": sent,
            "answers": answers,
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
                .map(|jpeg| engine.generate(&[jpeg], &prompt, Some(&grammar), MAX_ANSWER_TOKENS))
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
        fn generate(
            &self,
            images: &[&[u8]],
            prompt: &str,
            _: Option<&str>,
            _: u32,
        ) -> Result<String> {
            assert!(
                !images.is_empty() && images.iter().all(|i| !i.is_empty()) && !prompt.is_empty()
            );
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
        models.set_loader(Arc::new(move |_, _, _, _| Ok(fake_loaded(&answer, "CPU"))));
    }

    pub fn fake_loaded(answer: &str, runs_on: &str) -> Loaded {
        Loaded {
            engine: Arc::new(FakeVision(answer.to_string())),
            runs_on: runs_on.into(),
            gpu_layers: 0,
            layers: 32,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A GPU that fails to take the model leaves it on the CPU, and Settings says why; another device loads it
    /// again, and deleting the model lets go of it.
    #[tokio::test]
    async fn falls_back_to_the_cpu_and_reloads_on_another_device() {
        let dir = tempfile::tempdir().unwrap();
        let local = LocalModels::new(dir.path().to_path_buf());
        testing::install_fake(&local, "{}");
        let loads = Arc::new(Mutex::new(Vec::new()));
        let seen = loads.clone();
        local.set_loader(Arc::new(move |_, _, _, device| {
            seen.lock().unwrap().push(device.clone());
            match device {
                Device::Named(_) => Err(Error::msg("out of GPU memory")),
                Device::Auto => Ok(testing::fake_loaded("{}", "Test GPU")),
                Device::Cpu => Ok(testing::fake_loaded("{}", "CPU")),
            }
        }));
        let id = catalog::DEFAULT_MODEL;
        local.engine(id).await.unwrap();
        local.engine(id).await.unwrap();
        assert_eq!(*loads.lock().unwrap(), [Device::Auto], "loaded once");
        let active = local.device_list().active.unwrap();
        assert_eq!(
            (active.runs_on.as_str(), active.fallback),
            ("Test GPU", None)
        );

        local.set_device(Device::from_setting("Vulkan1"));
        local.engine(id).await.unwrap();
        let active = local.device_list().active.unwrap();
        assert_eq!(active.runs_on, "CPU");
        assert!(active.fallback.unwrap().contains("out of GPU memory"));
        assert_eq!(
            loads.lock().unwrap()[1..],
            [Device::Named("Vulkan1".into()), Device::Cpu]
        );

        local.delete(id).await.unwrap();
        assert!(local.device_list().active.is_none());
    }

    #[test]
    fn device_settings() {
        assert_eq!(Device::from_setting(""), Device::Auto);
        assert_eq!(Device::from_setting("cpu"), Device::Cpu);
        assert_eq!(Device::from_setting("CUDA0"), Device::Named("CUDA0".into()));
    }

    /// llama.cpp finds at least the CPU: with `gpu-backends` that means its backend modules were found and loaded.
    #[cfg(feature = "local-model")]
    #[test]
    fn lists_the_cpu() {
        let devices = devices();
        assert!(devices.iter().any(|d| d.kind == "cpu"), "{devices:?}");
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

    /// Scores a model on labelled cards. `CARDLOADER_TRUTH` is a CSV with the image (`front` or `file`), optionally
    /// `back`, the card's `name` (or `player`) and `number`; image paths are relative to the CSV. With
    /// `CARDLOADER_BACKS=1` the backs go to models that read them. Prints each card and how often the name and the
    /// number agree, counted as "Compare with local model" counts them.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore]
    async fn real_model_scores_cards() {
        let models = std::env::var("CARDLOADER_MODELS").expect("CARDLOADER_MODELS");
        let truth = PathBuf::from(std::env::var("CARDLOADER_TRUTH").expect("CARDLOADER_TRUTH"));
        let backs = std::env::var("CARDLOADER_BACKS").is_ok_and(|v| v == "1");
        // The session's "Identify as": tcg, sport or comics (default auto).
        let kind = IdMode::parse(&std::env::var("CARDLOADER_KIND").unwrap_or_default());
        let local = LocalModels::new(PathBuf::from(models));
        let id = test_model();
        assert!(local.is_ready(&id), "model files missing");
        let dir = truth.parent().unwrap().to_path_buf();
        let mut reader = csv::Reader::from_path(&truth).unwrap();
        let headers = reader.headers().unwrap().clone();
        let col = |row: &csv::StringRecord, names: &[&str]| {
            names
                .iter()
                .find_map(|n| {
                    headers
                        .iter()
                        .position(|h| h == *n)
                        .and_then(|i| row.get(i))
                })
                .unwrap_or_default()
                .to_string()
        };
        let (mut cards, mut names, mut numbers, mut seconds) = (0, 0, 0, 0.0);
        for row in reader.records() {
            let row = row.unwrap();
            let front = dir.join(col(&row, &["front", "file"]));
            let back = Some(col(&row, &["back"]))
                .filter(|b| backs && !b.is_empty())
                .map(|b| dir.join(b));
            let (r, raw) = local
                .identify(&id, &front, back.as_deref(), kind)
                .await
                .unwrap();
            let (name, number) = (col(&row, &["name", "player"]), col(&row, &["number"]));
            let name_ok = compare::names_match(&name, &r.card.name);
            let number_ok = compare::numbers_match(&number, &r.card.card_number);
            cards += 1;
            names += name_ok as usize;
            numbers += number_ok as usize;
            seconds += raw["seconds"].as_f64().unwrap();
            let mark = |ok: bool| if ok { "ok" } else { "--" };
            println!(
                "{:<34} {:>5.2}s {} {:<26} {} {:<10} | {} {} {}",
                col(&row, &["front", "file"]),
                raw["seconds"].as_f64().unwrap(),
                mark(name_ok),
                r.card.name,
                mark(number_ok),
                r.card.card_number,
                name,
                number,
                raw["images"]
            );
        }
        println!(
            "SCORE {id} {} backs={backs}: names {names}/{cards}, numbers {numbers}/{cards}, {:.2} s per card",
            truth.display(),
            seconds / cards.max(1) as f64
        );
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
        // The session's "Identify as", as in `real_model_scores_cards`.
        let kind = IdMode::parse(&std::env::var("CARDLOADER_KIND").unwrap_or_default());
        for path in paths.iter().filter(|p| imaging::is_supported(p)) {
            let (r, raw) = local.identify(&id, path, None, kind).await.unwrap();
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
                       "name": r.card.name, "number": r.card.card_number, "set": r.card.set_name,
                       "year": r.card.year, "game": r.card.subcategory})
            );
        }
    }
}
