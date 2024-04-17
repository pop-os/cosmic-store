use std::{collections::HashMap, sync::OnceLock, time::Instant};

static STATS: OnceLock<HashMap<String, u64>> = OnceLock::new();

pub fn monthly_downloads(id: &str) -> Option<u64> {
    let stats = STATS.get_or_init(|| {
        let start = Instant::now();
        match bitcode::decode::<HashMap<String, u64>>(include_bytes!(
            "../res/flathub-stats-2024-03.bitcode-v0-6"
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
    stats.get(id.trim_end_matches(".desktop")).copied()
}
