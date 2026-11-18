use std::collections::VecDeque;
use std::fs;
use std::io::{Cursor, Read};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, Mutex};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const INDEX_URL: &str = "https://cdn.swofty.net/launcher/preview/clips.json";

const CACHE_FRAMES: usize = 16;
const AHEAD: usize = 6;

/// A decoded frame: RGBA8 sRGB.
#[derive(Debug, Clone)]
pub struct Frame {
    pub index: usize,
    pub width: u32,
    pub height: u32,
    pub rgba: Vec<u8>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipInfo {
    pub title: String,
    pub fps: f32,
    pub width: u32,
    pub height: u32,
    pub frames: usize,
    #[serde(default)]
    pub view: String,
}

struct Shared {
    dir: PathBuf,
    info: ClipInfo,
    cache: Mutex<VecDeque<Arc<Frame>>>,
}

impl Shared {
    fn cached(&self, index: usize) -> Option<Arc<Frame>> {
        let cache = self.cache.lock().ok()?;
        cache.iter().find(|f| f.index == index).cloned()
    }

    fn decode(&self, index: usize) -> Result<Arc<Frame>, String> {
        if let Some(frame) = self.cached(index) {
            return Ok(frame);
        }
        let path = self.dir.join(format!("{index:04}.webp"));
        let bytes = fs::read(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let image = image::load_from_memory_with_format(&bytes, image::ImageFormat::WebP)
            .map_err(|e| format!("{}: {e}", path.display()))?
            .into_rgba8();
        let frame = Arc::new(Frame {
            index,
            width: image.width(),
            height: image.height(),
            rgba: image.into_raw(),
        });
        if let Ok(mut cache) = self.cache.lock()
            && cache.iter().all(|f| f.index != index)
        {
            cache.push_back(frame.clone());
            while cache.len() > CACHE_FRAMES {
                cache.pop_front();
            }
        }
        Ok(frame)
    }
}

/// A clip folder: `clip.json` and `0000.webp` …. Frames decode on demand and the frames after the
/// requested one decode on a background thread.
pub struct Clip {
    pub info: ClipInfo,
    shared: Arc<Shared>,
    ahead: Sender<usize>,
}

impl Clip {
    pub fn open(dir: &Path) -> Result<Self, String> {
        let path = dir.join("clip.json");
        let text = fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?;
        let info: ClipInfo = serde_json::from_str(&text).map_err(|e| format!("{}: {e}", path.display()))?;
        if info.frames == 0 || info.fps.partial_cmp(&0.0) != Some(std::cmp::Ordering::Greater) {
            return Err(format!("{}: no frames or no frame rate", path.display()));
        }
        let shared = Arc::new(Shared {
            dir: dir.to_path_buf(),
            info: info.clone(),
            cache: Mutex::new(VecDeque::new()),
        });
        let (ahead, wanted) = mpsc::channel::<usize>();
        let worker = Arc::downgrade(&shared);
        std::thread::Builder::new()
            .name("preview-clip".into())
            .spawn(move || {
                while let Ok(mut index) = wanted.recv() {
                    while let Ok(newer) = wanted.try_recv() {
                        index = newer;
                    }
                    let Some(shared) = worker.upgrade() else {
                        break;
                    };
                    for i in index..(index + AHEAD).min(shared.info.frames) {
                        if shared.decode(i).is_err() {
                            break;
                        }
                    }
                }
            })
            .map_err(|e| e.to_string())?;
        Ok(Self { info, shared, ahead })
    }

    pub fn len(&self) -> usize {
        self.info.frames
    }

    pub fn is_empty(&self) -> bool {
        self.info.frames == 0
    }

    pub fn fps(&self) -> f32 {
        self.info.fps
    }

    pub fn frame(&self, index: usize) -> Result<Arc<Frame>, String> {
        if index >= self.info.frames {
            return Err(format!("frame {index} is past the end of the clip"));
        }
        let frame = self.shared.decode(index)?;
        let _ = self.ahead.send(index + 1);
        Ok(frame)
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ClipEntry {
    pub id: String,
    pub title: String,
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

#[derive(Debug, Clone, PartialEq)]
pub struct CachedClip {
    pub id: String,
    pub title: String,
    pub dir: PathBuf,
}

/// The clips cached under `<data>/cache/preview/` and the index of the ones on the CDN.
/// The clip ids in the order of the last index read, which `cached` lists them in.
const ORDER: &str = "order.txt";

pub struct ClipStore {
    cache: PathBuf,
    index_url: String,
}

impl ClipStore {
    pub fn new(cache: &Path) -> Self {
        Self {
            cache: cache.to_path_buf(),
            index_url: INDEX_URL.into(),
        }
    }

    pub fn with_index_url(mut self, url: &str) -> Self {
        self.index_url = url.into();
        self
    }

    pub fn dir(&self, id: &str) -> PathBuf {
        self.cache.join(id)
    }

    pub fn cached(&self) -> Vec<CachedClip> {
        let Ok(read) = fs::read_dir(&self.cache) else {
            return Vec::new();
        };
        let mut clips: Vec<CachedClip> = read
            .flatten()
            .filter_map(|entry| {
                let dir = entry.path();
                let id = entry.file_name().to_str()?.to_string();
                let text = fs::read_to_string(dir.join("clip.json")).ok()?;
                let info: ClipInfo = serde_json::from_str(&text).ok()?;
                Some(CachedClip {
                    id,
                    title: info.title,
                    dir,
                })
            })
            .collect();
        let order: Vec<String> = fs::read_to_string(self.cache.join(ORDER))
            .map(|t| t.lines().map(str::to_owned).collect())
            .unwrap_or_default();
        let rank = |id: &str| order.iter().position(|o| o == id).unwrap_or(usize::MAX);
        clips.sort_by(|a, b| rank(&a.id).cmp(&rank(&b.id)).then_with(|| a.id.cmp(&b.id)));
        clips
    }

    pub fn index(&self) -> Result<Vec<ClipEntry>, String> {
        let mut response = ureq::get(&self.index_url)
            .header("User-Agent", "swofty-launcher")
            .call()
            .map_err(|e| format!("clip index: {e}"))?;
        let text = response
            .body_mut()
            .read_to_string()
            .map_err(|e| format!("clip index: {e}"))?;
        let entries: Vec<ClipEntry> = serde_json::from_str(&text).map_err(|e| format!("clip index: {e}"))?;
        let order: Vec<&str> = entries.iter().map(|e| e.id.as_str()).collect();
        fs::create_dir_all(&self.cache).ok();
        fs::write(self.cache.join(ORDER), order.join("\n")).ok();
        Ok(entries)
    }

    /// Downloads a clip zip, checks its sha256 and unpacks it into `<cache>/<id>/`.
    /// `progress` gets the bytes read so far and the total.
    pub fn download(&self, entry: &ClipEntry, mut progress: impl FnMut(u64, u64)) -> Result<PathBuf, String> {
        if entry.id.is_empty() || entry.id.contains(['/', '\\']) || entry.id.starts_with('.') {
            return Err(format!("bad clip id {:?}", entry.id));
        }
        let response = ureq::get(&entry.url)
            .header("User-Agent", "swofty-launcher")
            .call()
            .map_err(|e| format!("{}: {e}", entry.url))?;
        let total = response
            .headers()
            .get("content-length")
            .and_then(|v| v.to_str().ok())
            .and_then(|v| v.parse().ok())
            .unwrap_or(entry.size);
        let mut body = response.into_body();
        let mut reader = body.as_reader();
        let mut bytes = Vec::with_capacity(total as usize);
        let mut chunk = [0u8; 64 * 1024];
        loop {
            let n = reader.read(&mut chunk).map_err(|e| e.to_string())?;
            if n == 0 {
                break;
            }
            bytes.extend_from_slice(&chunk[..n]);
            progress(bytes.len() as u64, total);
        }
        self.install(entry, &bytes)
    }

    /// Checks `bytes` against the entry's sha256 and unpacks them into `<cache>/<id>/`.
    pub fn install(&self, entry: &ClipEntry, bytes: &[u8]) -> Result<PathBuf, String> {
        let digest = Sha256::digest(bytes);
        let hex: String = digest.iter().map(|b| format!("{b:02x}")).collect();
        if !hex.eq_ignore_ascii_case(&entry.sha256) {
            return Err(format!(
                "clip {} does not match its checksum (got {hex})",
                entry.id
            ));
        }
        let mut zip = zip::ZipArchive::new(Cursor::new(bytes)).map_err(|e| e.to_string())?;
        let staging = self.cache.join(format!(".{}.part", entry.id));
        let _ = fs::remove_dir_all(&staging);
        fs::create_dir_all(&staging).map_err(|e| e.to_string())?;
        for i in 0..zip.len() {
            let mut file = zip.by_index(i).map_err(|e| e.to_string())?;
            if file.is_dir() {
                continue;
            }
            let Some(name) = file
                .enclosed_name()
                .and_then(|p| p.file_name().map(|n| n.to_owned()))
            else {
                continue;
            };
            let mut data = Vec::new();
            file.read_to_end(&mut data).map_err(|e| e.to_string())?;
            fs::write(staging.join(name), data).map_err(|e| e.to_string())?;
        }
        if !staging.join("clip.json").exists() {
            let _ = fs::remove_dir_all(&staging);
            return Err(format!("clip {} has no clip.json", entry.id));
        }
        let dir = self.dir(&entry.id);
        let _ = fs::remove_dir_all(&dir);
        fs::rename(&staging, &dir).map_err(|e| e.to_string())?;
        Ok(dir)
    }
}
