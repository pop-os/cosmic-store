use appstream::{
    enums::{Bundle, Icon, ImageKind, Launchable},
    xmltree, Component,
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
        return Err(format!("maximum recursion level reached").into());
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
}

// Replaced Screenshot due to skip_field not supported in bitcode
#[derive(Clone, Debug, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct AppScreenshot {
    pub caption: String,
    pub url: String,
}

#[derive(Clone, Debug, Hash, Eq, PartialEq, bitcode::Decode, bitcode::Encode)]
pub struct AppInfo {
    pub origin_opt: Option<String>,
    pub name: String,
    pub summary: String,
    pub description: String,
    pub pkgnames: Vec<String>,
    pub categories: Vec<String>,
    pub desktop_ids: Vec<String>,
    pub flatpak_refs: Vec<String>,
    pub icons: Vec<AppIcon>,
    pub screenshots: Vec<AppScreenshot>,
}

impl AppInfo {
    pub fn new(origin_opt: Option<&str>, component: Component, locale: &str) -> Self {
        let name = get_translatable(&component.name, locale);
        let summary = component
            .summary
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
                _ => None,
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
                        url: image.url.to_string(),
                    });
                    break;
                }
            }
        }

        Self {
            origin_opt: origin_opt.map(|x| x.to_string()),
            name: name.to_string(),
            summary: summary.to_string(),
            description,
            pkgnames: component.pkgname.map_or(Vec::new(), |x| vec![x]),
            categories,
            desktop_ids,
            flatpak_refs,
            icons,
            screenshots,
        }
    }
}
