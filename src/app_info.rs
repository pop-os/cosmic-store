use appstream::{
    Component,
    enums::{Bundle, ComponentKind, Icon, ImageKind, Launchable, ProjectUrl, Provide},
    xmltree,
};
use std::{error::Error, fmt::Write};

fn get_translatable<'a>(translatable: &'a appstream::TranslatableString, locale: &str) -> &'a str {
    match translatable.get_for_locale(locale) {
        Some(some) => some.as_str(),
        None => match translatable.get_default() {
            Some(some) => some.as_str(),
            None => "",
        },
    }
}

//TODO: handle p tags with xml:lang
fn get_markup_translatable<'a>(
    translatable: &'a appstream::MarkupTranslatableString,
    locale: &str,
) -> &'a str {
    match translatable.get_for_locale(locale) {
        Some(some) => some.as_str(),
        None => match translatable.get_default() {
            Some(some) => some.as_str(),
            None => "",
        },
    }
}

fn write_node(
    s: &mut String,
    node: &xmltree::XMLNode,
    recursion: usize,
) -> Result<(), Box<dyn Error>> {
    if recursion >= 4 {
        return Err("maximum recursion level reached".to_string().into());
    }
    match node {
        xmltree::XMLNode::Element(element) => match element.name.as_str() {
            //TODO: actually style these
            "b" | "em" => {
                for child in element.children.iter() {
                    write_node(s, child, recursion + 1)?;
                }
            }
            "code" | "pre" => {
                for child in element.children.iter() {
                    write_node(s, child, recursion + 1)?;
                }
            }
            "li" => {
                for child in element.children.iter() {
                    write_node(s, child, recursion + 1)?;
                }
            }
            "ol" | "p" | "ul" => {
                if !s.is_empty() {
                    writeln!(s)?;
                }
                for (i, child) in element.children.iter().enumerate() {
                    if element.name == "ol" {
                        write!(s, "{:2}. ", i + 1)?;
                    } else if element.name == "ul" {
                        write!(s, " * ")?;
                    }
                    write_node(s, child, recursion + 1)?;
                }
                if !s.ends_with("\n") {
                    writeln!(s)?;
                }
            }
            _ => {
                return Err(format!("unknown element {:?}", element.name).into());
            }
        },
        xmltree::XMLNode::Text(text) => {
            for line in text.trim().lines() {
                writeln!(s, "{}", line.trim())?;
            }
        }
        _ => {
            return Err(format!("unknown node {:?}", node).into());
        }
    }
    Ok(())
}

fn convert_markup(markup: &str) -> Result<String, Box<dyn Error>> {
    let mut s = String::new();
    for node in xmltree::Element::parse_all(markup.as_bytes())? {
        write_node(&mut s, &node, 0)?;
    }
    Ok(s)
}

// Replaced Icon due to skip_field not supported in bitcode
#[derive(Clone, Debug, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum AppIcon {
    Cached(String, Option<u32>, Option<u32>, Option<u32>),
    Stock(String),
    Remote(String, Option<u32>, Option<u32>, Option<u32>),
    Local(String, Option<u32>, Option<u32>, Option<u32>),
}

#[derive(Clone, Debug, Default, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum AppKind {
    #[default]
    DesktopApplication,
    Addon,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum AppProvide {
    Id(String),
    MediaType(String),
}

// Replaced Release due to skip_field not supported in bitcode
#[derive(Clone, Debug, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct AppRelease {
    pub timestamp: Option<i64>,
    pub version: String,
    pub description: Option<String>,
    pub url: Option<String>,
}

// Replaced Screenshot due to skip_field not supported in bitcode
#[derive(Clone, Debug, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct AppScreenshot {
    pub caption: String,
    pub url: String,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum AppUrl {
    BugTracker(String),
    Contact(String),
    Donation(String),
    Faq(String),
    Help(String),
    Homepage(String),
    Translate(String),
}

/// Wayland socket support level based on Flatpak metadata
#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum WaylandSupport {
    Native,
    #[default]
    Fallback,
    X11Only,
    Unknown,
}

/// Application framework detection
#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum AppFramework {
    #[default]
    Native,
    GTK3,
    GTK4,
    Qt5,
    Qt6,
    QtWebEngine,
    Electron,
    Unknown,
}

/// Risk level for Wayland compatibility
#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub enum RiskLevel {
    #[default]
    Low,
    Medium,
    High,
    Critical,
}

/// Wayland compatibility information
#[derive(Clone, Copy, Debug, Default, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct WaylandCompatibility {
    pub support: WaylandSupport,
    pub framework: AppFramework,
    pub risk_level: RiskLevel,
}

impl WaylandCompatibility {
    /// Decode an 8-bit Wayland compatibility bitcode.
    ///
    /// # Bitcode Format
    /// - Bits 0-1: Wayland Support (00=Unknown, 01=XWayland, 10=Native)
    /// - Bits 2-5: Framework (0001=GTK3, 0010=GTK4, 0011=Qt5, 0100=Qt6, 0101=Electron, etc.)
    /// - Bits 6-7: Risk Level (00=Low, 01=Medium, 10=High, 11=Critical)
    ///
    /// # Example Bitcodes
    /// - 0x0A = GTK4 + Native + Low (GNOME apps)
    /// - 0x06 = GTK3 + Native + Low (older GNOME apps)
    /// - 0x52 = Qt6 + Native + Medium (modern KDE apps)
    /// - 0x4E = Qt5 + Native + Medium (older KDE apps)
    /// - 0x96 = Electron + Native + High (Electron apps)
    /// - 0xC1 = X11-only + Critical (legacy apps)
    pub fn decode_bitcode(bitcode: u8) -> Self {
        // Bits 0-1: Wayland Support
        let support = match bitcode & 0b00000011 {
            0b00 => WaylandSupport::Unknown,
            0b01 => WaylandSupport::Fallback, // XWayland -> Fallback
            0b10 => WaylandSupport::Native,
            _ => WaylandSupport::Unknown,
        };

        // Bits 2-5: Framework
        let framework = match (bitcode >> 2) & 0b00001111 {
            0x01 => AppFramework::GTK3,
            0x02 => AppFramework::GTK4,
            0x03 => AppFramework::Qt5,
            0x04 => AppFramework::Qt6,
            0x05 => AppFramework::Electron,
            0x06 => AppFramework::QtWebEngine,
            0x07 => AppFramework::Unknown, // SDL2 not in enum, use Unknown
            _ => AppFramework::Unknown,
        };

        // Bits 6-7: Risk Level
        let risk_level = match (bitcode >> 6) & 0b00000011 {
            0b00 => RiskLevel::Low,
            0b01 => RiskLevel::Medium,
            0b10 => RiskLevel::High,
            0b11 => RiskLevel::Critical,
            _ => RiskLevel::Low,
        };

        WaylandCompatibility {
            support,
            framework,
            risk_level,
        }
    }
}

#[cfg(test)]
mod wayland_bitcode_tests {
    use super::*;

    #[test]
    fn test_decode_gtk4_native_low() {
        let compat = WaylandCompatibility::decode_bitcode(0x0A);
        assert_eq!(compat.framework, AppFramework::GTK4);
        assert_eq!(compat.support, WaylandSupport::Native);
        assert_eq!(compat.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_decode_gtk3_native_low() {
        let compat = WaylandCompatibility::decode_bitcode(0x06);
        assert_eq!(compat.framework, AppFramework::GTK3);
        assert_eq!(compat.support, WaylandSupport::Native);
        assert_eq!(compat.risk_level, RiskLevel::Low);
    }

    #[test]
    fn test_decode_electron_native_high() {
        let compat = WaylandCompatibility::decode_bitcode(0x96);
        assert_eq!(compat.framework, AppFramework::Electron);
        assert_eq!(compat.support, WaylandSupport::Native);
        assert_eq!(compat.risk_level, RiskLevel::High);
    }

    #[test]
    fn test_decode_qt6_native_medium() {
        let compat = WaylandCompatibility::decode_bitcode(0x52);
        assert_eq!(compat.framework, AppFramework::Qt6);
        assert_eq!(compat.support, WaylandSupport::Native);
        assert_eq!(compat.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_decode_qt5_native_medium() {
        let compat = WaylandCompatibility::decode_bitcode(0x4E);
        assert_eq!(compat.framework, AppFramework::Qt5);
        assert_eq!(compat.support, WaylandSupport::Native);
        assert_eq!(compat.risk_level, RiskLevel::Medium);
    }

    #[test]
    fn test_decode_x11_only_critical() {
        let compat = WaylandCompatibility::decode_bitcode(0xC1);
        // 0xC1 = 11000001
        // Bits 0-1: 01 = Fallback (XWayland)
        // Bits 2-5: 0000 = Unknown
        // Bits 6-7: 11 = Critical
        assert_eq!(compat.framework, AppFramework::Unknown);
        assert_eq!(compat.support, WaylandSupport::Fallback);
        assert_eq!(compat.risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_all_risk_levels() {
        assert_eq!(WaylandCompatibility::decode_bitcode(0x0A).risk_level, RiskLevel::Low);
        assert_eq!(WaylandCompatibility::decode_bitcode(0x52).risk_level, RiskLevel::Medium);
        assert_eq!(WaylandCompatibility::decode_bitcode(0x96).risk_level, RiskLevel::High);
        assert_eq!(WaylandCompatibility::decode_bitcode(0xC1).risk_level, RiskLevel::Critical);
    }

    #[test]
    fn test_all_frameworks() {
        // GTK3: 0x06 = 00000110 (bits 2-5 = 0001)
        assert_eq!(WaylandCompatibility::decode_bitcode(0x06).framework, AppFramework::GTK3);

        // GTK4: 0x0A = 00001010 (bits 2-5 = 0010)
        assert_eq!(WaylandCompatibility::decode_bitcode(0x0A).framework, AppFramework::GTK4);

        // Qt5: 0x0E = 00001110 (bits 2-5 = 0011)
        assert_eq!(WaylandCompatibility::decode_bitcode(0x0E).framework, AppFramework::Qt5);

        // Qt6: 0x12 = 00010010 (bits 2-5 = 0100)
        assert_eq!(WaylandCompatibility::decode_bitcode(0x12).framework, AppFramework::Qt6);

        // Electron: 0x16 = 00010110 (bits 2-5 = 0101)
        assert_eq!(WaylandCompatibility::decode_bitcode(0x16).framework, AppFramework::Electron);
    }

    #[test]
    fn test_all_support_levels() {
        // Unknown: bits 0-1 = 00
        let compat = WaylandCompatibility::decode_bitcode(0x00);
        assert_eq!(compat.support, WaylandSupport::Unknown);

        // Fallback (XWayland): bits 0-1 = 01
        let compat = WaylandCompatibility::decode_bitcode(0x01);
        assert_eq!(compat.support, WaylandSupport::Fallback);

        // Native: bits 0-1 = 10
        let compat = WaylandCompatibility::decode_bitcode(0x02);
        assert_eq!(compat.support, WaylandSupport::Native);
    }
}

#[derive(Clone, Debug, Default, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct AppInfo {
    pub source_id: String,
    pub source_name: String,
    pub origin_opt: Option<String>,
    pub name: String,
    pub summary: String,
    pub kind: AppKind,
    pub developer_name: String,
    pub description: String,
    pub license_opt: Option<String>,
    pub pkgnames: Vec<String>,
    pub package_paths: Vec<String>,
    pub categories: Vec<String>,
    pub desktop_ids: Vec<String>,
    pub flatpak_refs: Vec<String>,
    pub icons: Vec<AppIcon>,
    pub provides: Vec<AppProvide>,
    pub releases: Vec<AppRelease>,
    pub screenshots: Vec<AppScreenshot>,
    pub urls: Vec<AppUrl>,
    pub monthly_downloads: u64,
    pub verified: bool,
    pub wayland_compat: Option<WaylandCompatibility>,
}

impl AppInfo {
    pub fn new(
        source_id: &str,
        source_name: &str,
        origin_opt: Option<&str>,
        component: Component,
        locale: &str,
        monthly_downloads: u64,
        verified: bool,
        wayland_compat: Option<WaylandCompatibility>,
    ) -> Self {
        let name = get_translatable(&component.name, locale);
        let summary = component
            .summary
            .as_ref()
            .map_or("", |x| get_translatable(x, locale));
        let kind = match component.kind {
            ComponentKind::DesktopApplication => AppKind::DesktopApplication,
            ComponentKind::Addon => AppKind::Addon,
            _ => {
                log::warn!("unknown component kind {:?}", component.kind);
                AppKind::default()
            }
        };
        let developer_name = component
            .developer_name
            .as_ref()
            .map_or("", |x| get_translatable(x, locale));
        let description_markup = component
            .description
            .as_ref()
            .map_or("", |x| get_markup_translatable(x, locale));
        let description = match convert_markup(description_markup) {
            Ok(ok) => ok,
            Err(err) => {
                log::warn!(
                    "failed to parse description of {:?} from {:?}: {}",
                    component.id,
                    origin_opt,
                    err
                );
                String::new()
            }
        };
        let categories = component
            .categories
            .into_iter()
            .map(|category| category.to_string())
            .collect();
        let desktop_ids = component
            .launchables
            .into_iter()
            .filter_map(|launchable| match launchable {
                Launchable::DesktopId(desktop_id) => Some(desktop_id),
                _ => None,
            })
            .collect();
        let flatpak_refs = component
            .bundles
            .into_iter()
            .filter_map(|bundle| match bundle {
                Bundle::Flatpak { reference, .. } => Some(reference),
                _ => None,
            })
            .collect();
        let icons = component
            .icons
            .into_iter()
            .filter_map(|icon| match icon {
                Icon::Cached {
                    path,
                    width,
                    height,
                    scale,
                } => Some(AppIcon::Cached(
                    path.to_str()?.to_string(),
                    width,
                    height,
                    scale,
                )),
                Icon::Stock(path) => Some(AppIcon::Stock(path)),
                Icon::Remote {
                    url,
                    width,
                    height,
                    scale,
                } => Some(AppIcon::Remote(url.into(), width, height, scale)),
                Icon::Local {
                    path,
                    width,
                    height,
                    scale,
                } => Some(AppIcon::Local(
                    path.to_str()?.to_string(),
                    width,
                    height,
                    scale,
                )),
            })
            .collect();
        let provides = component
            .provides
            .into_iter()
            .filter_map(|provide| {
                Some(match provide {
                    Provide::Id(value) => AppProvide::Id(value.0),
                    Provide::MediaType(value) => AppProvide::MediaType(value),
                    _ => return None,
                })
            })
            .collect();
        let releases = component
            .releases
            .into_iter()
            .map(|release| {
                let description = release.description.as_ref().and_then(|x| {
                    match convert_markup(get_markup_translatable(x, locale)) {
                        Ok(ok) => Some(ok),
                        Err(err) => {
                            //TODO: better handling of release description
                            log::info!(
                                "failed to parse description of release {:?} of {:?} from {:?}: {}",
                                release.version,
                                component.id,
                                origin_opt,
                                err
                            );
                            None
                        }
                    }
                });
                AppRelease {
                    timestamp: release.date.map(|date| date.timestamp()),
                    version: release.version,
                    description,
                    url: release.url.map(|url| url.into()),
                }
            })
            .collect();
        let mut screenshots = Vec::new();
        for screenshot in component.screenshots.into_iter() {
            //TODO: better handle multiple images per screenshot
            for image in screenshot.images.into_iter() {
                if matches!(image.kind, ImageKind::Source) {
                    screenshots.push(AppScreenshot {
                        caption: screenshot
                            .caption
                            .as_ref()
                            .map_or("", |x| get_translatable(x, locale))
                            .to_string(),
                        url: image.url.into(),
                    });
                    break;
                }
            }
        }
        let urls = component
            .urls
            .into_iter()
            .filter_map(|project_url| {
                Some(match project_url {
                    ProjectUrl::BugTracker(url) => AppUrl::BugTracker(url.into()),
                    ProjectUrl::Contact(url) => AppUrl::Contact(url.into()),
                    ProjectUrl::Donation(url) => AppUrl::Donation(url.into()),
                    ProjectUrl::Faq(url) => AppUrl::Faq(url.into()),
                    ProjectUrl::Help(url) => AppUrl::Help(url.into()),
                    ProjectUrl::Homepage(url) => AppUrl::Homepage(url.into()),
                    ProjectUrl::Translate(url) => AppUrl::Translate(url.into()),
                    _ => return None,
                })
            })
            .collect();

        Self {
            source_id: source_id.to_string(),
            source_name: source_name.to_string(),
            origin_opt: origin_opt.map(|x| x.to_string()),
            name: name.to_string(),
            summary: summary.to_string(),
            kind,
            developer_name: developer_name.to_string(),
            description,
            license_opt: component.project_license.map(|x| x.to_string()),
            pkgnames: component.pkgname.map_or(Vec::new(), |x| vec![x]),
            package_paths: Vec::new(),
            categories,
            desktop_ids,
            flatpak_refs,
            icons,
            provides,
            releases,
            screenshots,
            urls,
            monthly_downloads,
            verified,
            wayland_compat,
        }
    }

    /// Get Wayland compatibility information using heuristics.
    ///
    /// This method uses a multi-tier approach:
    /// 1. Pre-computed data from flathub-stats (when available)
    /// 2. Parse Flatpak metadata from disk for installed apps
    /// 3. Heuristic detection based on app metadata (categories, name, etc.)
    ///
    /// Returns:
    /// - `Some(WaylandCompatibility)` if compatibility data is available
    /// - `None` if not enough information to make a determination
    pub fn wayland_compat_lazy(&self) -> Option<WaylandCompatibility> {
        // First, try to use pre-computed data from flathub-stats
        if let Some(compat) = &self.wayland_compat {
            return Some(compat.clone());
        }

        // Second, try parsing metadata from disk for installed apps
        #[cfg(feature = "flatpak")]
        {
            use crate::backend::parse_flatpak_metadata;

            // Try to get app ID from desktop_ids or flatpak_refs
            if let Some(app_id_raw) = self.desktop_ids.first().or_else(|| self.flatpak_refs.first()) {
                // Strip .desktop suffix if present (desktop_ids have it, flatpak_refs don't)
                let app_id = app_id_raw.strip_suffix(".desktop").unwrap_or(app_id_raw);

                log::debug!("Checking Wayland compat for app: {} (app_id: {}, desktop_ids: {:?}, flatpak_refs: {:?})",
                    self.name, app_id, self.desktop_ids, self.flatpak_refs);

                // Try user installation first, then system
                if let Some(compat) = parse_flatpak_metadata(app_id, true)
                    .or_else(|| parse_flatpak_metadata(app_id, false))
                {
                    return Some(compat);
                }
            }
        }

        // Third, use heuristics for non-installed Flatpak apps
        if !self.flatpak_refs.is_empty() {
            log::debug!("Using heuristics for non-installed Flatpak app: {}", self.name);
            return self.heuristic_wayland_compat();
        }

        log::debug!("No Wayland compat data available for: {} (not a Flatpak or no metadata)", self.name);
        None
    }

    /// Heuristic-based Wayland compatibility detection for non-installed apps.
    ///
    /// This uses app metadata (categories, name, developer) to make educated guesses.
    /// Conservative approach: only returns data when reasonably confident.
    fn heuristic_wayland_compat(&self) -> Option<WaylandCompatibility> {
        // Check categories for framework hints
        let categories_lower: Vec<String> = self.categories.iter()
            .map(|c| c.to_lowercase())
            .collect();

        let name_lower = self.name.to_lowercase();
        let dev_lower = self.developer_name.to_lowercase();

        log::debug!("Heuristic check for {}: categories={:?}, dev={}, name={}",
            self.name, categories_lower, dev_lower, name_lower);

        // Detect GNOME/GTK apps (high confidence - Wayland native)
        if categories_lower.iter().any(|c| c.contains("gnome") || c.contains("gtk"))
            || dev_lower.contains("gnome")
            || name_lower.contains("gnome")
        {
            log::debug!("  -> Detected as GNOME/GTK app (Low risk)");
            return Some(WaylandCompatibility {
                support: WaylandSupport::Native,
                framework: AppFramework::GTK3,
                risk_level: RiskLevel::Low,
            });
        }

        // Detect KDE/Qt apps (medium confidence - may have issues)
        if categories_lower.iter().any(|c| c.contains("kde") || c.contains("qt"))
            || dev_lower.contains("kde")
            || name_lower.contains("kde")
        {
            return Some(WaylandCompatibility {
                support: WaylandSupport::Native,
                framework: AppFramework::Qt6, // Assume Qt6 for modern apps
                risk_level: RiskLevel::Medium,
            });
        }

        // Detect known problematic frameworks by name patterns
        if name_lower.contains("electron") || self.desktop_ids.iter().any(|id| id.to_lowercase().contains("electron")) {
            return Some(WaylandCompatibility {
                support: WaylandSupport::Native,
                framework: AppFramework::Electron,
                risk_level: RiskLevel::High,
            });
        }

        // For other Flatpak apps, we don't have enough info - return None
        // This is conservative: we only show badges when we're reasonably confident
        None
    }
}
