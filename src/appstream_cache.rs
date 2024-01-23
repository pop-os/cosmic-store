use appstream::{
    enums::{ComponentKind, Icon, Launchable},
    xmltree, Component, ParseError,
};
use cosmic::widget;
use flate2::read::GzDecoder;
use rayon::prelude::*;
use serde::Deserialize;
use std::{
    cmp,
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    sync::Arc,
    time::{Instant, SystemTime},
};

use crate::AppInfo;

const PREFIXES: &'static [&'static str] = &["/usr/share", "/var/lib", "/var/cache"];
const CATALOGS: &'static [&'static str] = &["swcatalog", "app-info"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppstreamCacheTag {
    /// When the file was last modified in seconds from the unix epoch
    pub modified: u64,
    /// Size of the file in bytes
    pub size: u64,
}

#[derive(Debug, Default)]
pub struct AppstreamCache {
    // Uses btreemap for stable sort order
    pub path_tags: BTreeMap<PathBuf, AppstreamCacheTag>,
    pub icons_paths: Vec<PathBuf>,
    pub infos: HashMap<String, Arc<AppInfo>>,
    pub pkgnames: HashMap<String, HashSet<String>>,
}

impl AppstreamCache {
    pub fn new<P: AsRef<Path>>(paths: &[P], icons_paths: Vec<PathBuf>, locale: &str) -> Self {
        let mut path_tags = BTreeMap::new();
        for path in paths.iter() {
            let canonical = match fs::canonicalize(path) {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to canonicalize {:?}: {}", path.as_ref(), err);
                    continue;
                }
            };

            let metadata = match fs::metadata(&canonical) {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to read metadata of {:?}: {}", canonical, err);
                    continue;
                }
            };

            let modified = match metadata.modified() {
                Ok(system_time) => match system_time.duration_since(SystemTime::UNIX_EPOCH) {
                    Ok(duration) => duration.as_secs(),
                    Err(err) => {
                        log::error!(
                            "failed to convert modified time of {:?} to unix epoch: {}",
                            canonical,
                            err
                        );
                        continue;
                    }
                },
                Err(err) => {
                    log::error!("failed to read modified time of {:?}: {}", canonical, err);
                    continue;
                }
            };

            let size = metadata.len();

            path_tags.insert(canonical, AppstreamCacheTag { modified, size });
        }

        let path_results: Vec<_> = path_tags
            .par_iter()
            .filter_map(|(path, _tag)| {
                let file_name = match path.file_name() {
                    Some(file_name_os) => match file_name_os.to_str() {
                        Some(some) => some,
                        None => {
                            log::error!("failed to convert to UTF-8: {:?}", file_name_os);
                            return None;
                        }
                    },
                    None => {
                        log::error!("path has no file name: {:?}", path);
                        return None;
                    }
                };

                //TODO: memory map?
                let mut file = match fs::File::open(&path) {
                    Ok(ok) => ok,
                    Err(err) => {
                        log::error!("failed to open {:?}: {}", path, err);
                        return None;
                    }
                };

                if file_name.ends_with(".xml.gz") {
                    let mut gz = GzDecoder::new(&mut file);
                    match AppstreamCache::parse_xml(path, &mut gz, locale) {
                        Ok(infos) => Some(infos),
                        Err(err) => {
                            log::error!("failed to parse {:?}: {}", path, err);
                            None
                        }
                    }
                } else if file_name.ends_with(".yml.gz") {
                    let mut gz = GzDecoder::new(&mut file);
                    match AppstreamCache::parse_yaml(path, &mut gz, locale) {
                        Ok(infos) => Some(infos),
                        Err(err) => {
                            log::error!("failed to parse {:?}: {}", path, err);
                            None
                        }
                    }
                } else if file_name.ends_with(".xml") {
                    match AppstreamCache::parse_xml(path, &mut file, locale) {
                        Ok(infos) => Some(infos),
                        Err(err) => {
                            log::error!("failed to parse {:?}: {}", path, err);
                            None
                        }
                    }
                } else if file_name.ends_with(".yml") {
                    match AppstreamCache::parse_yaml(path, &mut file, locale) {
                        Ok(infos) => Some(infos),
                        Err(err) => {
                            log::error!("failed to parse {:?}: {}", path, err);
                            None
                        }
                    }
                } else {
                    log::error!("unknown appstream file type: {:?}", path);
                    None
                }
            })
            .collect();

        let mut appstream_cache = Self::default();
        appstream_cache.path_tags = path_tags;
        appstream_cache.icons_paths = icons_paths;
        for infos in path_results {
            for (id, info) in infos {
                if let Some(pkgname) = &info.pkgname {
                    appstream_cache
                        .pkgnames
                        .entry(pkgname.clone())
                        .or_insert_with(|| HashSet::new())
                        .insert(id.clone());
                }
                match appstream_cache.infos.insert(id.clone(), info) {
                    Some(_old) => {
                        //TODO: merge based on priority
                        log::debug!("found duplicate info {}", id);
                    }
                    None => {}
                }
            }
        }

        appstream_cache
    }

    pub fn system(locale: &str) -> Self {
        let mut paths = Vec::new();
        let mut icons_paths = Vec::new();
        //TODO: get using xdg dirs?
        for prefix in PREFIXES {
            let prefix_path = Path::new(prefix);
            if !prefix_path.is_dir() {
                continue;
            }

            for catalog in CATALOGS {
                let catalog_path = prefix_path.join(catalog);
                if !catalog_path.is_dir() {
                    continue;
                }

                for format in &["xml", "yaml"] {
                    let format_path = catalog_path.join(format);
                    if !format_path.is_dir() {
                        continue;
                    }

                    let readdir = match fs::read_dir(&format_path) {
                        Ok(ok) => ok,
                        Err(err) => {
                            log::error!("failed to read directory {:?}: {}", format_path, err);
                            continue;
                        }
                    };

                    for entry_res in readdir {
                        let entry = match entry_res {
                            Ok(ok) => ok,
                            Err(err) => {
                                log::error!(
                                    "failed to read entry in directory {:?}: {}",
                                    format_path,
                                    err
                                );
                                continue;
                            }
                        };

                        paths.push(entry.path());
                    }
                }

                let icons_path = catalog_path.join("icons");
                if icons_path.is_dir() {
                    icons_paths.push(icons_path);
                }
            }
        }

        AppstreamCache::new(&paths, icons_paths, locale)
    }

    pub fn icon_path(
        &self,
        origin_opt: Option<&str>,
        name: &Path,
        width_opt: Option<u32>,
        height_opt: Option<u32>,
        scale_opt: Option<u32>,
    ) -> Option<PathBuf> {
        //TODO: what to do with no origin?
        let origin = origin_opt?;
        //TODO: what to do with no width or height?
        let width = width_opt?;
        let height = height_opt?;
        let size = match scale_opt {
            //TODO: should a scale of 0 or 1 not add @scale?
            Some(scale) => format!("{}x{}@{}", width, height, scale),
            None => format!("{}x{}", width, height),
        };

        for icons_path in self.icons_paths.iter() {
            let icon_path = icons_path.join(origin).join(&size).join(name);
            if icon_path.is_file() {
                return Some(icon_path);
            }
        }

        None
    }

    pub fn icon(&self, info: &AppInfo) -> widget::icon::Handle {
        let mut icon_opt = None;
        let mut cached_size = 0;
        for info_icon in info.icons.iter() {
            //TODO: support other types of icons
            match info_icon {
                Icon::Cached {
                    path,
                    width,
                    height,
                    scale,
                } => {
                    let size = cmp::min(width.unwrap_or(0), height.unwrap_or(0));
                    if size < cached_size {
                        // Skip if size is less than cached size
                        continue;
                    }
                    if let Some(icon_path) =
                        self.icon_path(info.origin_opt.as_deref(), path, *width, *height, *scale)
                    {
                        icon_opt = Some(widget::icon::from_path(icon_path));
                        cached_size = size;
                    }
                }
                Icon::Stock(stock) => {
                    if cached_size != 0 {
                        // Skip if a cached icon was found
                        continue;
                    }
                    icon_opt = Some(widget::icon::from_name(stock.clone()).size(128).handle());
                }
                _ => {}
            }
        }
        icon_opt.unwrap_or_else(|| {
            widget::icon::from_name("package-x-generic")
                .size(128)
                .handle()
        })
    }

    fn parse_xml<R: Read>(
        path: &Path,
        reader: R,
        locale: &str,
    ) -> Result<Vec<(String, Arc<AppInfo>)>, Box<dyn Error>> {
        let start = Instant::now();
        //TODO: just running this and not saving the results makes a huge memory leak!
        let e = xmltree::Element::parse(reader)?;
        let _version = e
            .attributes
            .get("version")
            .ok_or_else(|| ParseError::missing_attribute("version", "collection"))?;
        let origin_opt = e.attributes.get("origin");
        let _arch_opt = e.attributes.get("architecture");
        let infos: Vec<_> = e
            .children
            .par_iter()
            .filter_map(|node| {
                if let xmltree::XMLNode::Element(ref e) = node {
                    if &*e.name == "component" {
                        match Component::try_from(e) {
                            Ok(component) => {
                                if component.kind != ComponentKind::DesktopApplication {
                                    // Skip anything that is not a desktop application
                                    //TODO: should we allow more components?
                                    return None;
                                }

                                let id = component.id.to_string();
                                return Some((
                                    id,
                                    Arc::new(AppInfo::new(
                                        origin_opt.map(|x| x.as_str()),
                                        component,
                                        locale,
                                    )),
                                ));
                            }
                            Err(err) => {
                                log::error!(
                                    "failed to parse {:?} in {:?}: {}",
                                    e.get_child("id")
                                        .and_then(|x| appstream::AppId::try_from(x).ok()),
                                    path,
                                    err
                                );
                            }
                        }
                    }
                }
                None
            })
            .collect();
        let duration = start.elapsed();
        log::info!(
            "loaded {} items from {:?} in {:?}",
            infos.len(),
            path,
            duration
        );
        Ok(infos)
    }

    fn parse_yaml<R: Read>(
        path: &Path,
        reader: R,
        locale: &str,
    ) -> Result<Vec<(String, Arc<AppInfo>)>, Box<dyn Error>> {
        let start = Instant::now();
        let mut origin_opt = None;
        let mut infos = Vec::new();
        //TODO: par_iter?
        for (doc_i, doc) in serde_yaml::Deserializer::from_reader(reader).enumerate() {
            let value = match serde_yaml::Value::deserialize(doc) {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to parse document {} in {:?}: {}", doc_i, path, err);
                    continue;
                }
            };
            if doc_i == 0 {
                origin_opt = value["Origin"].as_str().map(|x| x.to_string());
            } else {
                match Component::deserialize(&value) {
                    Ok(mut component) => {
                        if component.kind != ComponentKind::DesktopApplication {
                            // Skip anything that is not a desktop application
                            //TODO: should we allow more components?
                            continue;
                        }

                        //TODO: move to appstream crate
                        if let Some(icons) = value["Icon"].as_mapping() {
                            for (key, icon) in icons.iter() {
                                match key.as_str() {
                                    Some("cached") => match icon.as_sequence() {
                                        Some(sequence) => {
                                            for cached in sequence {
                                                match cached["name"].as_str() {
                                                    Some(name) => {
                                                        component.icons.push(Icon::Cached {
                                                            //TODO: add prefix?
                                                            path: PathBuf::from(name),
                                                            //TODO: handle parsing errors for these numbers
                                                            width: cached["width"]
                                                                .as_u64()
                                                                .and_then(|x| x.try_into().ok()),
                                                            height: cached["height"]
                                                                .as_u64()
                                                                .and_then(|x| x.try_into().ok()),
                                                            scale: cached["scale"]
                                                                .as_u64()
                                                                .and_then(|x| x.try_into().ok()),
                                                        });
                                                    }
                                                    None => {
                                                        log::warn!(
                                                        "unsupported cached icon {:?} for {:?} in {:?}",
                                                        cached,
                                                        component.id,
                                                        path
                                                    );
                                                    }
                                                }
                                            }
                                        }
                                        None => {
                                            log::warn!(
                                                "unsupported cached icons {:?} for {:?} in {:?}",
                                                icon,
                                                component.id,
                                                path
                                            );
                                        }
                                    },
                                    Some("remote") => {
                                        // For now we just ignore remote icons
                                        log::debug!(
                                            "ignoring remote icons {:?} for {:?} in {:?}",
                                            icon,
                                            component.id,
                                            path
                                        );
                                    }
                                    Some("stock") => match icon.as_str() {
                                        Some(stock) => {
                                            component.icons.push(Icon::Stock(stock.to_string()));
                                        }
                                        None => {
                                            log::warn!(
                                                "unsupported stock icon {:?} for {:?} in {:?}",
                                                icon,
                                                component.id,
                                                path
                                            );
                                        }
                                    },
                                    _ => {
                                        log::warn!(
                                            "unsupported icon kind {:?} for {:?} in {:?}",
                                            key,
                                            component.id,
                                            path
                                        );
                                    }
                                }
                            }
                        }

                        if let Some(launchables) = value["Launchable"].as_mapping() {
                            for (key, launchable) in launchables.iter() {
                                match key.as_str() {
                                    Some("desktop-id") => match launchable.as_sequence() {
                                        Some(sequence) => {
                                            for desktop_id in sequence {
                                                match desktop_id.as_str() {
                                                    Some(desktop_id) => {
                                                        component.launchables.push(
                                                            Launchable::DesktopId(
                                                                desktop_id.to_string(),
                                                            ),
                                                        );
                                                    }
                                                    None => {
                                                        log::warn!(
                                                        "unsupported desktop-id launchable {:?} for {:?} in {:?}",
                                                        desktop_id,
                                                        component.id,
                                                        path
                                                    );
                                                    }
                                                }
                                            }
                                        }
                                        None => {
                                            log::warn!(
                                                "unsupported desktop-id launchables {:?} for {:?} in {:?}",
                                                launchable,
                                                component.id,
                                                path
                                            );
                                        }
                                    },
                                    _ => {
                                        log::warn!(
                                            "unsupported launchable kind {:?} for {:?} in {:?}",
                                            key,
                                            component.id,
                                            path
                                        );
                                    }
                                }
                            }
                        }

                        let id = component.id.to_string();
                        infos.push((
                            id,
                            Arc::new(AppInfo::new(origin_opt.as_deref(), component, locale)),
                        ));
                    }
                    Err(err) => {
                        log::error!("failed to parse {:?} in {:?}: {}", value["ID"], path, err);
                    }
                }
            }
        }
        let duration = start.elapsed();
        log::info!(
            "loaded {} items from {:?} in {:?}",
            infos.len(),
            path,
            duration
        );
        Ok(infos)
    }
}
