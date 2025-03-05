use std::{collections::HashMap, error::Error, fs};

use app_id::AppId;
#[path = "../../src/app_id.rs"]
mod app_id;

#[derive(serde::Deserialize)]
pub struct Stats {
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
    let year = 2025;
    let month = 2;
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
        for (id, archs) in stats.refs {
            for (_arch, (downloads, _updates)) in archs {
                *ref_downloads.entry(AppId::new(&id)).or_insert(0) += downloads;
            }
        }
    }

    let bitcode = bitcode::encode(&ref_downloads);
    fs::write(format!("res/flathub-stats.bitcode-v0-6"), &bitcode)?;

    Ok(())
}
