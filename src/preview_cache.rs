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

/// Maximum cache size in bytes (250 MB)
const MAX_CACHE_BYTES: u64 = 250 * 1024 * 1024;

/// Convert a URL to a cache file path using a hash
fn url_to_path(url: &str) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());
    dirs::cache_dir().map(|dir| dir.join("cosmic-store/previews").join(&hash))
}

/// Get cached image data from disk (returns None if not cached)
pub fn get_cached(url: &str) -> Option<Vec<u8>> {
    let path = url_to_path(url)?;
    std::fs::read(&path).ok()
}

/// Save data to disk cache
pub fn save_to_cache(url: &str, data: &[u8]) -> std::io::Result<()> {
    let path = url_to_path(url)
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "no cache directory"))?;
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(path, data)
}

/// Global download queue (LIFO for prioritizing recently viewed items)
static PENDING: Mutex<Vec<String>> = Mutex::new(Vec::new());

/// Notification for waking the background downloader
static NOTIFY: OnceLock<Notify> = OnceLock::new();

fn notify() -> &'static Notify {
    NOTIFY.get_or_init(Notify::new)
}

/// Queue a URL for background download if not already cached
pub fn queue(url: &str) {
    // Skip if already cached
    if url_to_path(url).is_some_and(|p| p.exists()) {
        return;
    }
    if let Ok(mut queue) = PENDING.lock() {
        // Avoid duplicates (cheap linear scan since queue stays small)
        if !queue.iter().any(|u| u == url) {
            queue.push(url.to_string());
            notify().notify_one();
        }
    }
}

/// Take URLs to download (LIFO order, up to MAX_BATCH_SIZE)
pub fn take_pending() -> Vec<String> {
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

/// Evict least-recently-accessed entries until total cache size is within the limit.
pub fn enforce_size_limit() {
    let Some(cache_dir) = dirs::cache_dir().map(|d| d.join("cosmic-store/previews")) else {
        return;
    };
    let Ok(entries) = std::fs::read_dir(&cache_dir) else {
        return;
    };

    // Collect all files with their size and access time
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
        let accessed = metadata.accessed().unwrap_or(
            metadata.modified().unwrap_or(SystemTime::UNIX_EPOCH),
        );
        total_size += size;
        files.push((path, size, accessed));
    }

    if total_size <= MAX_CACHE_BYTES {
        return;
    }

    // Sort by access time ascending (oldest accessed first)
    files.sort_by_key(|(_, _, accessed)| *accessed);

    let mut evicted = 0usize;
    for (path, size, _) in &files {
        if total_size <= MAX_CACHE_BYTES {
            break;
        }
        if std::fs::remove_file(path).is_ok() {
            total_size -= size;
            evicted += 1;
        }
    }
    if evicted > 0 {
        log::info!(
            "preview cache: evicted {} files to stay within {} MB limit",
            evicted,
            MAX_CACHE_BYTES / (1024 * 1024)
        );
    }
}
