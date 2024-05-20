use std::{collections::HashMap, sync::OnceLock, time::Instant};

use crate::AppId;

static STATS: OnceLock<HashMap<AppId, u64>> = OnceLock::new();

pub fn monthly_downloads(id: &AppId) -> Option<u64> {
    let stats = STATS.get_or_init(|| {
        let start = Instant::now();
        match bitcode::decode::<HashMap<AppId, u64>>(include_bytes!(
            "../res/flathub-stats-2024-04.bitcode-v0-6"
        )) {
            Ok(ok) => {
                let elapsed = start.elapsed();
                log::info!("loaded flathub statistics in {:?}", elapsed);
                ok
            }
            Err(err) => {
                log::warn!("failed to load flathub statistics: {}", err);
                HashMap::new()
            }
        }
    });
    stats.get(&id).copied()
}
