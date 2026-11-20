// SPDX-License-Identifier: GPL-3.0-only

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::PathBuf,
    sync::{Mutex, OnceLock},
    time::SystemTime,
};
use tokio::sync::Notify;

/// Maximum number of URLs to take from queue per tick
const MAX_BATCH_SIZE: usize = 5;

/// A folder of downloaded files keyed by URL and app version, trimmed to a size limit.
pub struct DiskCache {
    dir: &'static str,
    max_bytes: u64,
}

pub const PREVIEWS: DiskCache = DiskCache {
    dir: "previews",
    max_bytes: 250 * 1024 * 1024,
};

pub const ICONS: DiskCache = DiskCache {
    dir: "remote-icons",
    max_bytes: 50 * 1024 * 1024,
};

impl DiskCache {
    pub fn path(&self, url: &str, version: &str) -> Option<PathBuf> {
        let mut hasher = DefaultHasher::new();
        url.hash(&mut hasher);
        version.hash(&mut hasher);
        let hash = format!("{:016x}", hasher.finish());
        dirs::cache_dir().map(|dir| dir.join("cosmic-store").join(self.dir).join(&hash))
    }

    pub fn get_cached(&self, url: &str, version: &str) -> Option<Vec<u8>> {
        std::fs::read(self.path(url, version)?).ok()
    }

    pub fn save_to_cache(&self, url: &str, version: &str, data: &[u8]) -> std::io::Result<()> {
        let path = self.path(url, version).ok_or_else(|| {
            std::io::Error::new(std::io::ErrorKind::NotFound, "no cache directory")
        })?;
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, data)
    }

    pub fn enforce_size_limit(&self) {
        let Some(cache_dir) = dirs::cache_dir().map(|d| d.join("cosmic-store").join(self.dir))
        else {
            return;
        };
        let Ok(entries) = std::fs::read_dir(&cache_dir) else {
            return;
        };

        let mut files: Vec<(PathBuf, u64, SystemTime)> = Vec::new();
        let mut total_size: u64 = 0;

        for entry in entries.filter_map(Result::ok) {
            let path = entry.path();
            if !path.is_file() {
                continue;
            }
            let Ok(metadata) = path.metadata() else {
                continue;
            };
            let size = metadata.len();
            let accessed = metadata
                .accessed()
                .unwrap_or(metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH));
            total_size += size;
            files.push((path, size, accessed));
        }

        if total_size <= self.max_bytes {
            return;
        }

        files.sort_by_key(|(_, _, accessed)| *accessed);

        let mut evicted = 0usize;
        for (path, size, _) in &files {
            if total_size <= self.max_bytes {
                break;
            }
            if std::fs::remove_file(path).is_ok() {
                total_size -= size;
                evicted += 1;
            }
        }
        if evicted > 0 {
            log::info!(
                "{} cache: evicted {} files to stay within {} MB limit",
                self.dir,
                evicted,
                self.max_bytes / (1024 * 1024)
            );
        }
    }
}

/// Global download queue of `(url, version)` (LIFO for prioritizing recently
/// viewed items)
static PENDING: Mutex<Vec<(String, String)>> = Mutex::new(Vec::new());

/// Notification for waking the background downloader
static NOTIFY: OnceLock<Notify> = OnceLock::new();

fn notify() -> &'static Notify {
    NOTIFY.get_or_init(Notify::new)
}

/// Queue a URL (for the given app version) for background download if not
/// already cached
pub fn queue(url: &str, version: &str) {
    // Skip if already cached
    if PREVIEWS.path(url, version).is_some_and(|p| p.exists()) {
        return;
    }
    if let Ok(mut queue) = PENDING.lock() {
        // Avoid duplicates (cheap linear scan since queue stays small)
        if !queue.iter().any(|(u, v)| u == url && v == version) {
            queue.push((url.to_string(), version.to_string()));
            notify().notify_one();
        }
    }
}

/// Take `(url, version)` pairs to download (LIFO order, up to MAX_BATCH_SIZE)
pub fn take_pending() -> Vec<(String, String)> {
    let Ok(mut queue) = PENDING.lock() else {
        return Vec::new();
    };
    let start = queue.len().saturating_sub(MAX_BATCH_SIZE);
    queue.split_off(start)
}

/// Wait for work to be queued. Returns immediately if work is already pending.
pub async fn wait_for_work() {
    // Check if there's already work pending
    if PENDING.lock().map(|q| !q.is_empty()).unwrap_or(false) {
        return;
    }
    notify().notified().await;
}
