use std::{collections::HashMap, sync::OnceLock, time::Instant};

use crate::app_info::WaylandCompatibility;
use crate::AppId;

// New format (v0-7) - downloads + compatibility
#[derive(bitcode::Decode)]
struct FlathubStatsV7 {
    downloads: HashMap<AppId, u64>,
    compatibility: HashMap<AppId, WaylandCompatibility>,
}

// Internal unified format
struct FlathubStats {
    downloads: HashMap<AppId, u64>,
    compatibility: HashMap<AppId, WaylandCompatibility>,
}

static STATS: OnceLock<FlathubStats> = OnceLock::new();

fn load_stats() -> &'static FlathubStats {
    STATS.get_or_init(|| {
        let start = Instant::now();

        // Try v0-7 first (with compatibility data) if it exists
        #[cfg(feature = "flathub-stats-v7")]
        {
            if let Ok(v7) = bitcode::decode::<FlathubStatsV7>(include_bytes!(
                "../res/flathub-stats.bitcode-v0-7"
            )) {
                let elapsed = start.elapsed();
                log::info!("loaded flathub statistics v0-7 in {:?}", elapsed);
                return FlathubStats {
                    downloads: v7.downloads,
                    compatibility: v7.compatibility,
                };
            }
        }

        // Use v0-6 (downloads only) - this is just a HashMap<AppId, u64>
        match bitcode::decode::<HashMap<AppId, u64>>(include_bytes!(
            "../res/flathub-stats.bitcode-v0-6"
        )) {
            Ok(downloads) => {
                let elapsed = start.elapsed();
                log::info!("loaded flathub statistics v0-6 in {:?}", elapsed);
                FlathubStats {
                    downloads,
                    compatibility: HashMap::new(),
                }
            }
            Err(err) => {
                log::warn!("failed to load flathub statistics: {}", err);
                FlathubStats {
                    downloads: HashMap::new(),
                    compatibility: HashMap::new(),
                }
            }
        }
    })
}

pub fn monthly_downloads(id: &AppId) -> Option<u64> {
    load_stats().downloads.get(id).copied()
}

pub fn wayland_compatibility(id: &AppId) -> Option<WaylandCompatibility> {
    load_stats().compatibility.get(id).cloned()
}
