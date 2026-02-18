use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    sync::{Arc, OnceLock, RwLock},
};

use chrono::{Duration, NaiveDate, Utc};

use crate::AppId;

static STATS: OnceLock<RwLock<Arc<HashMap<AppId, u64>>>> = OnceLock::new();

// --- Shared cache helpers ---

fn load_cache(path: &Path) -> Option<HashMap<AppId, u64>> {
    let compressed = std::fs::read(path).ok()?;
    let data = zstd::decode_all(compressed.as_slice()).ok()?;
    bitcode::decode(&data).ok()
}

fn save_cache(path: &Path, map: &HashMap<AppId, u64>) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir: {e}"))?;
    }
    let data = bitcode::encode(map);
    let compressed =
        zstd::encode_all(data.as_slice(), 3).map_err(|e| format!("compress: {e}"))?;
    std::fs::write(path, &compressed).map_err(|e| format!("write: {e}"))?;
    Ok(())
}

// --- Disk cache (merged 30-day stats) ---

fn cache_path() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("cosmic-store").join("flathub-stats.bitcode.zst"))
}

fn load_disk_cache() -> Option<HashMap<AppId, u64>> {
    let path = cache_path()?;
    let map = load_cache(&path)?;
    log::info!(
        "loaded disk-cached flathub stats ({} entries)",
        map.len(),
    );
    Some(map)
}

fn save_disk_cache(map: &HashMap<AppId, u64>) {
    let Some(path) = cache_path() else {
        log::warn!("no cache dir for flathub stats");
        return;
    };
    match save_cache(&path, map) {
        Ok(()) => log::info!("saved flathub stats cache ({} entries)", map.len()),
        Err(err) => log::warn!("failed to save flathub stats cache: {}", err),
    }
}

fn init_stats() -> RwLock<Arc<HashMap<AppId, u64>>> {
    let map = load_disk_cache().unwrap_or_default();
    RwLock::new(Arc::new(map))
}

pub fn monthly_downloads(id: &AppId) -> Option<u64> {
    let lock = STATS.get_or_init(init_stats);
    let map = lock.read().unwrap();
    map.get(id).copied()
}

/// Sort key for ordering apps by popularity (most downloads first).
pub fn download_sort_key(id: &AppId) -> i64 {
    -(monthly_downloads(id).unwrap_or(0) as i64)
}

pub fn update_stats(new: HashMap<AppId, u64>) {
    let lock = STATS.get_or_init(init_stats);
    let mut guard = lock.write().unwrap();
    *guard = Arc::new(new);
}

/// Returns true if stats have been loaded (from disk cache or network)
pub fn is_loaded() -> bool {
    let lock = STATS.get_or_init(init_stats);
    let map = lock.read().unwrap();
    !map.is_empty()
}

// --- Per-day cache functions ---

fn daily_cache_dir() -> Option<PathBuf> {
    dirs::cache_dir().map(|p| p.join("cosmic-store").join("flathub-stats"))
}

fn daily_cache_path(date: NaiveDate) -> Option<PathBuf> {
    daily_cache_dir().map(|d| d.join(format!("{}.bitcode.zst", date)))
}

fn load_daily_cache(date: NaiveDate) -> Option<HashMap<AppId, u64>> {
    load_cache(&daily_cache_path(date)?)
}

fn save_daily_cache(date: NaiveDate, map: &HashMap<AppId, u64>) {
    let Some(path) = daily_cache_path(date) else {
        return;
    };
    if let Err(err) = save_cache(&path, map) {
        log::warn!("failed to save daily cache for {}: {}", date, err);
    }
}

/// Returns dates (most-recent-first) that are missing from the daily cache.
pub fn missing_days() -> Vec<NaiveDate> {
    let today = Utc::now().date_naive();
    let mut missing = Vec::new();
    for days_ago in 1..=30 {
        let date = today - Duration::days(days_ago);
        if let Some(path) = daily_cache_path(date) {
            if !path.exists() {
                missing.push(date);
            }
        }
    }
    missing
}

/// Returns true if stats need work — either days are missing or the merged cache is gone.
pub fn needs_refresh() -> bool {
    !is_loaded() || !missing_days().is_empty()
}

/// Typed representation of a single day's Flathub stats JSON.
#[derive(serde::Deserialize)]
struct DailyStats {
    /// New format (2026+): { "app.id": { "CC": (downloads, updates) } }
    #[serde(default)]
    ref_by_country: HashMap<String, HashMap<String, (u64, u64)>>,
    /// Old format (pre-2026): { "app.id/arch": { "channel": (downloads, updates) } }
    #[serde(default)]
    refs: HashMap<String, HashMap<String, (u64, u64)>>,
}

/// Read all daily cache files for the last 30 days, sum into a merged map,
/// then save the merged disk cache and update the in-memory stats.
pub fn rebuild_merged_from_daily_caches() {
    let today = Utc::now().date_naive();
    let mut merged = HashMap::<AppId, u64>::new();
    let mut loaded = 0u32;
    for days_ago in 1..=30 {
        let date = today - Duration::days(days_ago);
        if let Some(day_map) = load_daily_cache(date) {
            for (id, count) in day_map {
                *merged.entry(id).or_insert(0) += count;
            }
            loaded += 1;
        }
    }
    log::info!(
        "rebuilt merged stats from {} daily caches ({} apps)",
        loaded,
        merged.len()
    );
    save_disk_cache(&merged);
    update_stats(merged);
}

/// Fetch one day's stats JSON from Flathub and save the daily cache.
/// Does NOT rebuild the merged stats — call `rebuild_merged_from_daily_caches()` separately.
pub async fn fetch_day(
    client: &reqwest::Client,
    date: NaiveDate,
) -> Result<(), Box<dyn std::error::Error + Send + Sync>> {
    let url = format!("https://flathub.org/stats/{}.json", date.format("%Y/%m/%d"));

    let resp = client.get(&url).send().await?;
    if !resp.status().is_success() {
        return Err(format!("HTTP {} for {}", resp.status(), url).into());
    }

    let body = resp.text().await?;
    let stats: DailyStats = serde_json::from_str(&body)?;

    let mut day_map = HashMap::<AppId, u64>::new();
    if !stats.ref_by_country.is_empty() {
        for (app_id, countries) in &stats.ref_by_country {
            let total: u64 = countries.values().map(|(dl, _)| *dl).sum();
            if total > 0 {
                *day_map.entry(AppId::new(app_id)).or_insert(0) += total;
            }
        }
    } else {
        for (r, archs) in &stats.refs {
            let id = r.split('/').next().unwrap_or(r);
            for (downloads, _) in archs.values() {
                *day_map.entry(AppId::new(id)).or_insert(0) += *downloads;
            }
        }
    }

    log::debug!("fetched stats for {}: {} apps", date, day_map.len());

    save_daily_cache(date, &day_map);

    Ok(())
}

/// Fetch multiple days concurrently. Saves each day's cache file.
/// Does NOT rebuild merged stats — call `rebuild_merged_from_daily_caches()` after.
pub async fn fetch_days(client: &reqwest::Client, dates: Vec<NaiveDate>) {
    let mut set = tokio::task::JoinSet::new();
    for date in dates {
        let client = client.clone();
        set.spawn(async move {
            if let Err(err) = fetch_day(&client, date).await {
                log::warn!("failed to fetch stats for {}: {}", date, err);
            }
        });
    }
    while set.join_next().await.is_some() {}
}

/// Remove daily cache files older than 31 days.
pub fn cleanup_old_daily_caches() {
    let Some(dir) = daily_cache_dir() else {
        return;
    };
    let entries = match std::fs::read_dir(&dir) {
        Ok(e) => e,
        Err(_) => return,
    };
    let cutoff = Utc::now().date_naive() - Duration::days(31);
    for entry in entries.flatten() {
        let name = entry.file_name();
        let Some(name_str) = name.to_str() else {
            continue;
        };
        let Some(date_str) = name_str.strip_suffix(".bitcode.zst") else {
            continue;
        };
        if let Ok(date) = NaiveDate::parse_from_str(date_str, "%Y-%m-%d") {
            if date < cutoff {
                if let Err(err) = std::fs::remove_file(entry.path()) {
                    log::warn!("failed to remove old daily cache {}: {}", name_str, err);
                } else {
                    log::debug!("removed old daily cache {}", name_str);
                }
            }
        }
    }
}
