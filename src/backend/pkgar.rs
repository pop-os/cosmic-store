use cosmic::widget;
use std::{collections::HashMap, error::Error, fmt::Write, fs, sync::Arc};

use super::{Backend, Package};
use crate::{AppId, AppInfo, AppUrl, AppstreamCache, Operation, OperationKind};

#[derive(Debug)]
pub struct Pkgar {
    appstream_caches: Vec<AppstreamCache>,
}

impl Pkgar {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        let source_id = "pkgar";
        //TODO: translate?
        let source_name = "System";
        Ok(Self {
            appstream_caches: vec![AppstreamCache::system(
                source_id.to_string(),
                source_name.to_string(),
                locale,
            )],
        })
    }
}

impl Backend for Pkgar {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>> {
        for appstream_cache in self.appstream_caches.iter_mut() {
            appstream_cache.reload();
        }
        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let appstream_cache = &self.appstream_caches[0];

        let mut system_packages = Vec::new();
        let mut packages = Vec::new();
        for entry_res in fs::read_dir("/pkg")? {
            let entry = entry_res?;
            let file_name_os = entry.file_name();
            let file_name = file_name_os.to_string_lossy();
            if file_name.ends_with(".pkgar_head") {
                let package_name = file_name.trim_end_matches(".pkgar_head");
                let version_opt = None; //TODO: get pkgar version
                println!("installed: {}", package_name);
                match appstream_cache.pkgnames.get(package_name) {
                    Some(ids) => {
                        for id in ids.iter() {
                            match appstream_cache.infos.get(&id) {
                                Some(info) => {
                                    packages.push(Package {
                                        id: id.clone(),
                                        icon: appstream_cache.icon(info),
                                        info: info.clone(),
                                        version: version_opt.unwrap_or("").to_string(),
                                        extra: HashMap::new(),
                                    });
                                }
                                None => {
                                    log::warn!("failed to find info {:?}", id);
                                }
                            }
                        }
                    }
                    None => {
                        // Ignore packages with no components
                        log::debug!("no components for package {}", package_name);
                        system_packages.push((
                            package_name.to_string(),
                            version_opt.unwrap_or("").to_string(),
                        ));
                    }
                }
            }
        }

        if !system_packages.is_empty() {
            let name = crate::fl!("system-packages");
            let summary = crate::fl!("system-packages-summary", count = system_packages.len());
            let mut description = String::new();
            let mut pkgnames = Vec::with_capacity(system_packages.len());
            for (package_name, version) in system_packages {
                let _ = writeln!(description, " * {}: {}", package_name, version);
                pkgnames.push(package_name);
            }
            //TODO: translate
            packages.push(Package {
                id: AppId::system(),
                icon: widget::icon::from_name("package-x-generic")
                    .size(128)
                    .handle(),
                //TODO: fill in more AppInfo fields
                info: Arc::new(AppInfo {
                    source_id: appstream_cache.source_id.clone(),
                    source_name: appstream_cache.source_name.clone(),
                    name,
                    summary,
                    description,
                    pkgnames,
                    ..Default::default()
                }),
                version: String::new(),
                extra: HashMap::new(),
            });
        }

        Ok(packages)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        Ok(Vec::new())
    }

    fn file_packages(&self, path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("Pkgar::file_packages not implemented".into())
    }

    fn operation(
        &self,
        op: &Operation,
        mut f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        Err("Pkgar::operation not implemented".into())
    }
}
