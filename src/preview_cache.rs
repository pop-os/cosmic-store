// SPDX-License-Identifier: GPL-3.0-only

use std::{
    hash::{DefaultHasher, Hash, Hasher},
    path::{Path, PathBuf},
    sync::{Mutex, OnceLock, atomic::AtomicU64, atomic::Ordering},
    time::{Duration, SystemTime},
};
use tokio::sync::Notify;

/// Maximum number of URLs to take from queue per tick
const MAX_BATCH_SIZE: usize = 5;

/// Cache TTL: 1 year in seconds
const CACHE_TTL_SECS: u64 = 365 * 24 * 60 * 60;

/// Minimum interval between scroll-based queue operations (ms)
const SCROLL_DEBOUNCE_MS: u64 = 100;

/// Convert a URL to a cache file path using a hash
fn url_to_path(url: &str) -> Option<PathBuf> {
    let mut hasher = DefaultHasher::new();
    url.hash(&mut hasher);
    let hash = format!("{:016x}", hasher.finish());
    // Use first 2 chars as subdirectory for filesystem performance
    dirs::cache_dir().map(|dir| {
        dir.join("cosmic-store/previews")
            .join(&hash[..2])
            .join(&hash)
    })
}

/// Check if a cached file is still valid (exists and not expired)
fn is_cache_valid(path: &Path) -> bool {
    let Ok(metadata) = path.metadata() else {
        return false;
    };
    let Ok(modified) = metadata.modified() else {
        return true; // Can't check age, assume valid
    };
    let age = SystemTime::now()
        .duration_since(modified)
        .unwrap_or(Duration::ZERO);
    age.as_secs() < CACHE_TTL_SECS
}

/// Get cached image data from disk (returns None if not cached or expired)
pub fn get_cached(url: &str) -> Option<Vec<u8>> {
    let path = url_to_path(url)?;
    if !is_cache_valid(&path) {
        let _ = std::fs::remove_file(&path);
        return None;
    }
    std::fs::read(path).ok()
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
    if url_to_path(url).is_some_and(|p| is_cache_valid(&p)) {
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

/// Debounce state for scroll-based queueing (stores epoch millis)
static LAST_SCROLL_QUEUE: AtomicU64 = AtomicU64::new(0);

/// Returns true if enough time has passed since last scroll queue operation.
/// Uses compare_exchange to avoid race conditions between concurrent calls.
pub fn should_queue_on_scroll() -> bool {
    let now = SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map(|d| d.as_millis() as u64)
        .unwrap_or(0);
    let last = LAST_SCROLL_QUEUE.load(Ordering::Relaxed);
    now.saturating_sub(last) >= SCROLL_DEBOUNCE_MS
        && LAST_SCROLL_QUEUE
            .compare_exchange(last, now, Ordering::Relaxed, Ordering::Relaxed)
            .is_ok()
}

/// Run cleanup of expired cache files
pub fn cleanup_expired() {
    let Some(cache_dir) = dirs::cache_dir().map(|d| d.join("cosmic-store/previews")) else {
        return;
    };
    let Ok(subdirs) = std::fs::read_dir(&cache_dir) else {
        return;
    };

    let mut removed = 0usize;
    for subdir in subdirs.filter_map(Result::ok) {
        let path = subdir.path();
        if !path.is_dir() {
            continue;
        }
        let Ok(files) = std::fs::read_dir(&path) else {
            continue;
        };
        for file in files.filter_map(Result::ok) {
            let file_path = file.path();
            if file_path.is_file()
                && !is_cache_valid(&file_path)
                && std::fs::remove_file(&file_path).is_ok()
            {
                removed += 1;
            }
        }
    }
    if removed > 0 {
        log::info!("preview cache: removed {} expired files", removed);
    }
}
