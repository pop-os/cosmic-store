use cosmic::widget;
use libflatpak::{gio::Cancellable, prelude::*, Installation, RefKind};
use std::{collections::HashMap, error::Error};

use super::{Backend, Package};
use crate::AppstreamCache;

#[derive(Debug)]
pub struct Flatpak {
    appstream_cache: AppstreamCache,
}

impl Flatpak {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        //TODO: should we support system installations?
        let inst = Installation::new_user(Cancellable::NONE)?;
        let mut paths = Vec::new();
        let mut icons_paths = Vec::new();
        for remote in inst.list_remotes(Cancellable::NONE)? {
            if let Some(appstream_dir) = remote.appstream_dir(None).and_then(|x| x.path()) {
                let xml_gz_path = appstream_dir.join("appstream.xml.gz");
                if xml_gz_path.is_file() {
                    paths.push(xml_gz_path);
                } else {
                    let xml_path = appstream_dir.join("appstream.xml");
                    if xml_path.is_file() {
                        paths.push(xml_path);
                    }
                }

                let icons_path = appstream_dir.join("icons");
                if icons_path.is_dir() {
                    icons_paths.push(icons_path);
                }
            }
        }

        // We don't store the installation because it is not Send
        Ok(Self {
            appstream_cache: AppstreamCache::new(paths, icons_paths, locale),
        })
    }
}

impl Backend for Flatpak {
    fn load_cache(&mut self) -> Result<(), Box<dyn Error>> {
        self.appstream_cache.reload("flatpak");
        Ok(())
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        //TODO: should we support system installations?
        let inst = Installation::new_user(Cancellable::NONE)?;
        let mut packages = Vec::new();
        //TODO: show non-desktop items?
        for r in inst.list_installed_refs_by_kind(RefKind::App, Cancellable::NONE)? {
            if let Some(id) = r.name() {
                let mut extra = HashMap::new();
                if let Some(arch) = r.arch() {
                    extra.insert("arch".to_string(), arch.to_string());
                }
                if let Some(branch) = r.branch() {
                    extra.insert("branch".to_string(), branch.to_string());
                }
                packages.push(Package {
                    id: id.to_string(),
                    //TODO: get icon from appstream data?
                    icon: widget::icon::from_name(id.to_string()).size(128).handle(),
                    name: r.appdata_name().unwrap_or(id).to_string(),
                    summary: r.appdata_summary().map_or(String::new(), |x| x.to_string()),
                    version: r.appdata_version().unwrap_or_default().to_string(),
                    origin_opt: r.origin().map(|x| x.to_string()),
                    extra,
                })
            }
        }
        Ok(packages)
    }

    fn info_cache(&self) -> &AppstreamCache {
        &self.appstream_cache
    }
}
