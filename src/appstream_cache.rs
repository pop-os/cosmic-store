use appstream::{enums::Icon, Collection, Component};
use cosmic::widget;
use flate2::read::GzDecoder;
use serde::Deserialize;
use std::{
    cmp,
    collections::{BTreeMap, HashMap, HashSet},
    error::Error,
    fs,
    io::Read,
    path::{Path, PathBuf},
    time::SystemTime,
};

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
    pub collections: HashMap<String, Collection>,
    pub pkgnames: HashMap<String, HashSet<String>>,
}

impl AppstreamCache {
    //TODO: make async?
    pub fn new() -> Self {
        // Uses btreemap for stable sort order
        let mut paths = BTreeMap::new();
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

                        let path = entry.path();
                        let canonical = match fs::canonicalize(&path) {
                            Ok(ok) => ok,
                            Err(err) => {
                                log::error!("failed to canonicalize {:?}: {}", path, err);
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
                            Ok(system_time) => {
                                match system_time.duration_since(SystemTime::UNIX_EPOCH) {
                                    Ok(duration) => duration.as_secs(),
                                    Err(err) => {
                                        log::error!(
                                            "failed to convert modified time of {:?} to unix epoch: {}",
                                            canonical,
                                            err
                                        );
                                        continue;
                                    }
                                }
                            }
                            Err(err) => {
                                log::error!(
                                    "failed to read modified time of {:?}: {}",
                                    canonical,
                                    err
                                );
                                continue;
                            }
                        };

                        let size = metadata.len();

                        paths.insert(canonical, AppstreamCacheTag { modified, size });
                    }
                }
            }
        }

        //TODO: save cache to disk and update when tags change
        let mut appstream_cache = Self::default();
        for (path, _tag) in paths.iter() {
            let file_name = match path.file_name() {
                Some(file_name_os) => match file_name_os.to_str() {
                    Some(some) => some,
                    None => {
                        log::error!("failed to convert to UTF-8: {:?}", file_name_os);
                        continue;
                    }
                },
                None => {
                    log::error!("path has no file name: {:?}", path);
                    continue;
                }
            };

            let mut file = match fs::File::open(&path) {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to open {:?}: {}", path, err);
                    continue;
                }
            };

            if file_name.ends_with(".xml.gz") {
                log::info!("Compressed XML: {:?}", path);
                let mut gz = GzDecoder::new(&mut file);
                //TODO: support XML
            } else if file_name.ends_with(".yml.gz") {
                log::info!("Compressed YAML: {:?}", path);
                let mut gz = GzDecoder::new(&mut file);
                match appstream_cache.parse_yml(path, &mut gz) {
                    Ok(()) => {}
                    Err(err) => {
                        log::error!("failed to parse {:?}: {}", path, err);
                    }
                }
            } else if file_name.ends_with(".xml") {
                log::info!("XML: {:?}", path);
                //TODO: support XML
            } else if file_name.ends_with(".yml") {
                log::info!("YAML: {:?}", path);
                match appstream_cache.parse_yml(path, &mut file) {
                    Ok(()) => {}
                    Err(err) => {
                        log::error!("failed to parse {:?}: {}", path, err);
                    }
                }
            } else {
                log::error!("unknown appstream file type: {:?}", path);
                continue;
            };
        }

        appstream_cache
    }

    pub fn icon_path(
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

                let icon_path = catalog_path
                    .join("icons")
                    .join(origin)
                    .join(&size)
                    .join(name);
                if icon_path.is_file() {
                    return Some(icon_path);
                }
            }
        }

        None
    }

    pub fn icon(origin_opt: Option<&str>, component: &Component) -> widget::icon::Handle {
        let mut icon_opt = None;
        let mut cached_size = 0;
        for component_icon in component.icons.iter() {
            //TODO: support other types of icons
            match component_icon {
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
                        AppstreamCache::icon_path(origin_opt, path, *width, *height, *scale)
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

    fn parse_yml<R: Read>(&mut self, path: &Path, reader: R) -> Result<(), Box<dyn Error>> {
        let mut version_opt = None;
        let mut origin_opt = None;
        for (doc_i, doc) in serde_yaml::Deserializer::from_reader(reader).enumerate() {
            let value = match serde_yaml::Value::deserialize(doc) {
                Ok(ok) => ok,
                Err(err) => {
                    log::error!("failed to parse document {} in {:?}: {}", doc_i, path, err);
                    continue;
                }
            };
            if doc_i == 0 {
                version_opt = value["Version"].as_str().map(|x| x.to_string());
                origin_opt = value["Origin"].as_str().map(|x| x.to_string());
            } else {
                match Component::deserialize(&value) {
                    Ok(mut component) => {
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

                        let id = component.id.to_string();
                        if let Some(pkgname) = &component.pkgname {
                            self.pkgnames
                                .entry(pkgname.clone())
                                .or_insert_with(|| HashSet::new())
                                .insert(id.clone());
                        }
                        match self.collections.insert(
                            id.clone(),
                            Collection {
                                //TODO: default version
                                version: version_opt.clone().unwrap_or_default(),
                                origin: origin_opt.clone(),
                                components: vec![component],
                                //TODO: architecture
                                architecture: None,
                            },
                        ) {
                            Some(_old) => {
                                //TODO: merge based on priority
                                log::debug!("found duplicate collection {}", id);
                            }
                            None => {}
                        }
                    }
                    Err(err) => {
                        log::error!("failed to parse {:?} in {:?}: {}", value["ID"], path, err);
                    }
                }
            }
        }
        //TODO: return collection or error
        Ok(())
    }
}
