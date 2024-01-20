use flate2::read::GzDecoder;
use serde::Deserialize;
use std::{collections::BTreeMap, fs, path::Path, time::SystemTime};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct AppstreamCacheTag {
    /// When the file was last modified in seconds from the unix epoch
    pub modified: u64,
    /// Size of the file in bytes
    pub size: u64,
}

pub struct AppstreamCache;

impl AppstreamCache {
    //TODO: make async?
    pub fn new() -> Self {
        // Uses btreemap for stable sort order
        let mut paths = BTreeMap::new();
        //TODO: get using xdg dirs?
        for prefix in &["/usr/share", "/var/lib", "/var/cache"] {
            let prefix_path = Path::new(prefix);
            if !prefix_path.is_dir() {
                continue;
            }

            for catalog in &["swcatalog", "app-info"] {
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
                println!("Compressed XML: {:?}", path);
                let mut gz = GzDecoder::new(&mut file);
                //TODO: support XML
            } else if file_name.ends_with(".yml.gz") {
                println!("Compressed YAML: {:?}", path);
                let mut gz = GzDecoder::new(&mut file);
                for doc in serde_yaml::Deserializer::from_reader(&mut gz) {
                    match serde_yaml::Value::deserialize(doc) {
                        Ok(value) => {
                            //println!("{:?}", value);
                        }
                        Err(err) => {
                            log::error!("failed to parse {:?}: {}", path, err);
                        }
                    }
                }
            } else if file_name.ends_with(".xml") {
                println!("XML: {:?}", path);
                //TODO: support XML
            } else if file_name.ends_with(".yml") {
                println!("YAML: {:?}", path);
                for doc in serde_yaml::Deserializer::from_reader(&mut file) {
                    match serde_yaml::Value::deserialize(doc) {
                        Ok(value) => {
                            //println!("{:?}", value);
                        }
                        Err(err) => {
                            log::error!("failed to parse {:?}: {}", path, err);
                        }
                    }
                }
            } else {
                log::error!("unknown appstream file type: {:?}", path);
                continue;
            };
        }

        Self
    }
}
