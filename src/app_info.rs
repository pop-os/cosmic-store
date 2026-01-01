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
        }
    }
}
