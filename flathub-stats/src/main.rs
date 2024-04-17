use std::{collections::HashMap, error::Error, fs};

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

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let year = 2024;
    let month = 3;
    let days = 31;

    let mut ref_downloads = HashMap::<String, u64>::new();
    for day in 1..=days {
        let stats = stats(year, month, day).await?;
        for (id, archs) in stats.refs {
            for (_arch, (downloads, _updates)) in archs {
                *ref_downloads.entry(id.clone()).or_insert(0) += downloads;
            }
        }
    }

    let bitcode = bitcode::encode(&ref_downloads);
    fs::write(
        format!("flathub-stats-{year}-{month:02}.bitcode-v0-6"),
        &bitcode,
    )?;

    Ok(())
}
