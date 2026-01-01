use std::{collections::HashMap, error::Error, fs};

use app_id::AppId;
#[path = "../../src/app_id.rs"]
mod app_id;

#[derive(serde::Deserialize)]
pub struct Stats {
    refs: HashMap<String, HashMap<String, (u64, u64)>>,
}

// Compatibility data structures (matching src/app_info.rs)
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
pub enum WaylandSupport {
    Native,
    Fallback,
    X11Only,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
pub enum AppFramework {
    Native,
    GTK3,
    GTK4,
    Qt5,
    Qt6,
    QtWebEngine,
    Electron,
    Unknown,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
pub enum RiskLevel {
    Low,
    Medium,
    High,
    Critical,
}

#[derive(Debug, Clone, serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
pub struct WaylandCompatibility {
    pub support: WaylandSupport,
    pub framework: AppFramework,
    pub risk_level: RiskLevel,
}

#[derive(serde::Serialize, serde::Deserialize, bitcode::Encode, bitcode::Decode)]
struct FlathubStats {
    downloads: HashMap<AppId, u64>,
    compatibility: HashMap<AppId, WaylandCompatibility>,
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

/// Fetch app manifest from Flathub GitHub repository
async fn fetch_manifest(app_id: &str) -> Result<serde_json::Value, Box<dyn Error>> {
    // Try multiple branches: master, main, stable
    let branches = ["master", "main", "stable"];

    for branch in branches {
        let url = format!(
            "https://raw.githubusercontent.com/flathub/{}/{}/{}.json",
            app_id, branch, app_id
        );

        match reqwest::get(&url).await {
            Ok(response) if response.status().is_success() => {
                let manifest = response.json().await?;
                return Ok(manifest);
            }
            _ => continue,
        }
    }

    Err(format!("Failed to fetch manifest for {}", app_id).into())
}

/// Parse finish-args to detect socket permissions
fn parse_compatibility(manifest: &serde_json::Value) -> WaylandCompatibility {
    let mut wayland = false;
    let mut x11 = false;
    let mut fallback_x11 = false;

    // Parse finish-args
    if let Some(finish_args) = manifest["finish-args"].as_array() {
        for arg in finish_args {
            if let Some(arg_str) = arg.as_str() {
                if arg_str.contains("--socket=wayland") {
                    wayland = true;
                } else if arg_str.contains("--socket=fallback-x11") {
                    fallback_x11 = true;
                } else if arg_str.contains("--socket=x11") {
                    x11 = true;
                }
            }
        }
    }

    // Detect framework from runtime, SDK, or modules
    let framework = detect_framework(manifest);

    // Determine support level
    let support = if wayland && !x11 && !fallback_x11 {
        WaylandSupport::Native
    } else if wayland && fallback_x11 {
        WaylandSupport::Fallback
    } else if wayland && x11 {
        WaylandSupport::Fallback
    } else if x11 && !wayland {
        WaylandSupport::X11Only
    } else {
        WaylandSupport::Unknown
    };

    // Calculate risk level
    let risk_level = calculate_risk_level(support, framework);

    WaylandCompatibility {
        support,
        framework,
        risk_level,
    }
}

/// Detect application framework from manifest
fn detect_framework(manifest: &serde_json::Value) -> AppFramework {
    let manifest_str = manifest.to_string().to_lowercase();

    // Check for problematic frameworks first
    if manifest_str.contains("qtwebengine") || manifest_str.contains("qt5-qtwebengine") {
        return AppFramework::QtWebEngine;
    }
    if manifest_str.contains("electron") {
        return AppFramework::Electron;
    }

    // Check for Qt versions
    if manifest_str.contains("qt6") || manifest_str.contains("kde6") {
        return AppFramework::Qt6;
    }
    if manifest_str.contains("qt5") || manifest_str.contains("kde5") {
        return AppFramework::Qt5;
    }

    // Check for GTK versions
    if manifest_str.contains("gtk-4") || manifest_str.contains("gnome-44") || manifest_str.contains("gnome-45") || manifest_str.contains("gnome-46") {
        return AppFramework::GTK4;
    }
    if manifest_str.contains("gtk-3") || manifest_str.contains("gnome-3") {
        return AppFramework::GTK3;
    }

    AppFramework::Native
}

/// Calculate risk level based on support and framework
fn calculate_risk_level(support: WaylandSupport, framework: AppFramework) -> RiskLevel {
    use RiskLevel::*;

    match (support, framework) {
        // X11-only = critical risk
        (WaylandSupport::X11Only, _) => Critical,

        // Qt WebEngine or Electron = high risk (known Wayland issues)
        (_, AppFramework::QtWebEngine) => High,
        (_, AppFramework::Electron) => High,

        // Qt6 with native Wayland = medium risk (version-dependent)
        (WaylandSupport::Native, AppFramework::Qt6) => Medium,

        // Fallback support = medium risk (may use XWayland)
        (WaylandSupport::Fallback, _) => Medium,

        // Qt5 = medium risk
        (WaylandSupport::Native, AppFramework::Qt5) => Medium,

        // Native Wayland with GTK3/GTK4 = low risk (works well)
        (WaylandSupport::Native, AppFramework::GTK3) => Low,
        (WaylandSupport::Native, AppFramework::GTK4) => Low,

        // Native Wayland with native framework = low risk
        (WaylandSupport::Native, AppFramework::Native) => Low,

        // Unknown = assume medium risk
        _ => Medium,
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn Error>> {
    let year = 2025;
    let month = 9;

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

    // Fetch download stats
    println!("Fetching download stats for {}/{}...", year, month);
    let mut ref_downloads = HashMap::<AppId, u64>::new();
    for day in 1..=days {
        let stats = stats(year, month, day).await?;
        for (r, archs) in stats.refs {
            for (_arch, (downloads, _updates)) in archs {
                let id = r.split('/').next().unwrap();
                *ref_downloads.entry(AppId::new(&id)).or_insert(0) += downloads;
            }
        }
    }
    println!("Fetched stats for {} unique apps", ref_downloads.len());

    // Fetch compatibility data for all apps
    println!("Fetching compatibility data for {} apps...", ref_downloads.len());
    let mut compatibility_data = HashMap::<AppId, WaylandCompatibility>::new();

    let mut successful = 0;
    let mut failed = 0;

    for (app_id, _downloads) in ref_downloads.iter() {
        let app_id_str = app_id.raw();

        match fetch_manifest(app_id_str).await {
            Ok(manifest) => {
                let compat = parse_compatibility(&manifest);
                compatibility_data.insert(app_id.clone(), compat);
                successful += 1;

                if successful % 100 == 0 {
                    println!("Processed {}/{} apps...", successful, ref_downloads.len());
                }
            }
            Err(e) => {
                // Not all apps have manifests on GitHub (some are EOL, renamed, etc.)
                // This is expected, so we just skip them
                failed += 1;
            }
        }

        // Rate limiting: don't hammer GitHub
        tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
    }

    println!("Successfully fetched {} manifests, {} failed", successful, failed);

    // Store both downloads and compatibility data
    let stats = FlathubStats {
        downloads: ref_downloads,
        compatibility: compatibility_data,
    };

    let bitcode = bitcode::encode(&stats);
    fs::write("res/flathub-stats.bitcode-v0-7", &bitcode)?;

    println!("Saved to res/flathub-stats.bitcode-v0-7");

    Ok(())
}
