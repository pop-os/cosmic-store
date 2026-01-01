use std::{collections::HashMap, sync::OnceLock, time::Instant};

use crate::app_info::WaylandCompatibility;
use crate::AppId;

#[derive(bitcode::Decode)]
struct FlathubStats {
    downloads: HashMap<AppId, u64>,
    compatibility: HashMap<AppId, WaylandCompatibility>,
}

static STATS: OnceLock<FlathubStats> = OnceLock::new();

pub fn monthly_downloads(id: &AppId) -> Option<u64> {
    let stats = STATS.get_or_init(|| {
        let start = Instant::now();
        // Try v0-7 first (with compatibility data), fallback to v0-6 (downloads only)
        match bitcode::decode::<FlathubStats>(include_bytes!(
            "../res/flathub-stats.bitcode-v0-6"
        )) {
            Ok(ok) => {
                let elapsed = start.elapsed();
                log::info!("loaded flathub statistics in {:?}", elapsed);
                ok
            }
            Err(err) => {
                log::warn!("failed to load flathub statistics: {}", err);
                FlathubStats {
                    downloads: HashMap::new(),
                    compatibility: HashMap::new(),
                }
            }
        }
    });
    stats.downloads.get(id).copied()
}

pub fn wayland_compatibility(id: &AppId) -> Option<WaylandCompatibility> {
    let stats = STATS.get_or_init(|| {
        let start = Instant::now();
        // Try v0-7 first (with compatibility data), fallback to v0-6 (downloads only)
        match bitcode::decode::<FlathubStats>(include_bytes!(
            "../res/flathub-stats.bitcode-v0-6"
        )) {
            Ok(ok) => {
                let elapsed = start.elapsed();
                log::info!("loaded flathub statistics in {:?}", elapsed);
                ok
            }
            Err(err) => {
                log::warn!("failed to load flathub statistics: {}", err);
                FlathubStats {
                    downloads: HashMap::new(),
                    compatibility: HashMap::new(),
                }
            }
        }
    });
    stats.compatibility.get(id).cloned()
}
