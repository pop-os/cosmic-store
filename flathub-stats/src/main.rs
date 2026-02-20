use std::{collections::HashMap, error::Error, fs};

use app_id::AppId;
#[path = "../../src/app_id.rs"]
mod app_id;

#[derive(serde::Deserialize)]
pub struct Stats {
    /// New format (2026+): { "app.id": { "CC": (downloads, updates) } }
    #[serde(default)]
    ref_by_country: HashMap<String, HashMap<String, (u64, u64)>>,
    /// Old format (pre-2026): { "app.id/arch": { "channel": (downloads, updates) } }
    #[serde(default)]
    refs: HashMap<String, HashMap<String, (u64, u64)>>,
}

async fn stats(year: u16, month: u8, day: u8) -> Result<Stats, Box<dyn Error>> {
    let url = format!("https://flathub.org/stats/{year}/{month:02}/{day:02}.json");
    println!("Downloading stats from {}", url);
    let body = reqwest::get(url).await?.text().await?;
    let stats = serde_json::from_str::<Stats>(&body)?;
    Ok(stats)
}

fn leap_year(year: u16) -> bool {
    (year % 4 == 0 && year % 100 != 0) || year % 400 == 0
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let year = 2026;
    let month = 1;
    let days = match month {
        1 | 3 | 5 | 7 | 8 | 10 | 12 => 31,
        4 | 6 | 9 | 11 => 30,
        2 => {
            if leap_year(year) {
                29
            } else {
                28
            }
        }
        _ => panic!("invalid month {}", month),
    };

    let mut ref_downloads = HashMap::<AppId, u64>::new();
    for day in 1..=days {
        let stats = stats(year, month, day).await?;
        if !stats.ref_by_country.is_empty() {
            for (app_id, countries) in stats.ref_by_country {
                let total: u64 = countries.values().map(|(dl, _)| *dl).sum();
                if total > 0 {
                    *ref_downloads.entry(AppId::new(&app_id)).or_insert(0) += total;
                }
            }
        } else {
            for (r, archs) in stats.refs {
                for (_arch, (downloads, _updates)) in archs {
                    let id = r.split('/').next().unwrap();
                    *ref_downloads.entry(AppId::new(&id)).or_insert(0) += downloads;
                }
            }
        }
    }

    let bitcode = bitcode::encode(&ref_downloads);
    fs::write(format!("res/flathub-stats.bitcode-v0-6"), &bitcode)?;

    println!("Wrote stats for {} apps", ref_downloads.len());

    Ok(())
}
