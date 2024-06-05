use cosmic::widget;
use libflatpak::{gio::Cancellable, prelude::*, Installation, Ref, Transaction};
use std::{
    cell::Cell,
    collections::HashMap,
    error::Error,
    fmt::Write,
    sync::{Arc, Mutex},
};

use super::{Backend, Package};
use crate::{AppId, AppInfo, AppstreamCache, Operation, OperationKind};

#[derive(Debug)]
pub struct Flatpak {
    appstream_caches: Vec<AppstreamCache>,
}

impl Flatpak {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        let mut appstream_caches = Vec::new();

        //TODO: should we support system installations?
        let inst = Installation::new_user(Cancellable::NONE)?;
        for remote in inst.list_remotes(Cancellable::NONE)? {
            let source_id = match remote.name() {
                Some(some) => some.to_string(),
                None => {
                    log::warn!("remote {:?} missing name", remote);
                    continue;
                }
            };

            let appstream_dir = match remote.appstream_dir(None).and_then(|x| x.path()) {
                Some(some) => some,
                None => {
                    log::warn!("remote {:?} missing appstream dir", remote);
                    continue;
                }
            };

            //TODO: also update if out of date?
            if !appstream_dir.is_dir() {
                log::info!("updating appstream data for remote {:?}", remote);
                match inst.update_appstream_sync(&source_id, None, Cancellable::NONE) {
                    Ok(()) => {}
                    Err(err) => {
                        log::warn!(
                            "failed to update appstream data for remote {:?}: {}",
                            remote,
                            err
                        );
                    }
                }
            }

            let mut paths = Vec::new();
            let xml_gz_path = appstream_dir.join("appstream.xml.gz");
            if xml_gz_path.is_file() {
                paths.push(xml_gz_path);
            } else {
                let xml_path = appstream_dir.join("appstream.xml");
                if xml_path.is_file() {
                    paths.push(xml_path);
                }
            }

            let mut icons_paths = Vec::new();
            let icons_path = appstream_dir.join("icons");
            if icons_path.is_dir() {
                match icons_path.into_os_string().into_string() {
                    Ok(ok) => icons_paths.push(ok),
                    Err(os_string) => {
                        log::error!("failed to convert {:?} to string", os_string)
                    }
                }
            }

            let source_name = match remote.title() {
                Some(title) => title.to_string(),
                None => source_id.clone(),
            };
            appstream_caches.push(AppstreamCache::new(
                source_id,
                source_name,
                paths,
                icons_paths,
                locale,
            ));
        }

        // We don't store the installation because it is not Send
        Ok(Self { appstream_caches })
    }

    fn ref_to_package<R: InstalledRefExt + RefExt>(&self, r: &R) -> Option<Package> {
        let id_raw = r.name()?;
        let id = AppId::new(&id_raw);
        let origin = r.origin()?;
        for appstream_cache in self.appstream_caches.iter() {
            if &appstream_cache.source_id != &origin {
                // Only show items from correct cache
                continue;
            }

            //TODO: better matching of .desktop suffix
            let info = match appstream_cache.infos.get(&id) {
                Some(some) => some,
                None => continue,
            };

            let mut extra = HashMap::new();
            if let Some(arch) = r.arch() {
                extra.insert("arch".to_string(), arch.to_string());
            }
            if let Some(branch) = r.branch() {
                extra.insert("branch".to_string(), branch.to_string());
            }

            return Some(Package {
                id: id.clone(),
                icon: appstream_cache.icon(info),
                info: info.clone(),
                version: r.appdata_version().unwrap_or_default().to_string(),
                extra,
            });
        }

        log::debug!("failed to find info for {:?} from {}", id, origin);
        None
    }

    fn refs_to_packages<R: InstalledRefExt + RefExt>(&self, rs: Vec<R>) -> Vec<Package> {
        let mut packages = Vec::new();
        let mut system_packages = Vec::new();
        for r in rs {
            match self.ref_to_package(&r) {
                Some(package) => {
                    packages.push(package);
                }
                None => {
                    system_packages.push((
                        r.format_ref().unwrap_or_default().to_string(),
                        r.appdata_version()
                            .or(r.branch())
                            .unwrap_or_default()
                            .to_string(),
                    ));
                }
            }
        }

        if !system_packages.is_empty() {
            //TODO: use correct appstream cache, or do not bother to specify it
            let appstream_cache = &self.appstream_caches[0];
            let name = "System Packages".to_string();
            let summary = format!(
                "{} package{}",
                system_packages.len(),
                if system_packages.len() == 1 { "" } else { "s" }
            );
            let mut description = String::new();
            let mut flatpak_refs = Vec::with_capacity(system_packages.len());
            for (flatpak_ref, version) in system_packages {
                let _ = writeln!(description, " * {}: {}", flatpak_ref, version);
                flatpak_refs.push(flatpak_ref);
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
                    origin_opt: None,
                    name,
                    summary,
                    developer_name: String::new(),
                    description,
                    pkgnames: Vec::new(),
                    categories: Vec::new(),
                    desktop_ids: Vec::new(),
                    flatpak_refs,
                    icons: Vec::new(),
                    releases: Vec::new(),
                    screenshots: Vec::new(),
                    monthly_downloads: 0,
                }),
                version: String::new(),
                extra: HashMap::new(),
            });
        }

        packages
    }
}

impl Backend for Flatpak {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>> {
        if refresh {
            //TODO: should we support system installations?
            let inst = Installation::new_user(Cancellable::NONE)?;
            for remote in inst.list_remotes(Cancellable::NONE)? {
                let Some(remote_name) = remote.name() else {
                    continue;
                };
                inst.update_remote_sync(&remote_name, Cancellable::NONE)?;
                inst.update_appstream_sync(&remote_name, None, Cancellable::NONE)?;
            }
        }

        for appstream_cache in self.appstream_caches.iter_mut() {
            appstream_cache.reload();
        }
        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        //TODO: should we support system installations?
        let inst = Installation::new_user(Cancellable::NONE)?;
        let packages = self.refs_to_packages(inst.list_installed_refs(Cancellable::NONE)?);
        Ok(packages)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        //TODO: should we support system installations?
        let inst = Installation::new_user(Cancellable::NONE)?;
        let packages =
            self.refs_to_packages(inst.list_installed_refs_for_update(Cancellable::NONE)?);
        Ok(packages)
    }

    fn file_packages(&self, path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("flatpak backend does not support loading details from a file".into())
    }

    fn operation(
        &self,
        op: &Operation,
        callback: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        let callback = Arc::new(Mutex::new(callback));
        //TODO: should we support system installations?
        let inst = Installation::new_user(Cancellable::NONE)?;
        let total_ops = Arc::new(Cell::new(0));
        let tx = Transaction::for_installation(&inst, Cancellable::NONE)?;
        {
            let total_ops = total_ops.clone();
            tx.connect_ready(move |tx| {
                total_ops.set(tx.operations().len());
                true
            });
        }
        let started_ops = Arc::new(Cell::new(0));
        tx.connect_new_operation(move |_, op, progress| {
            let current_op = started_ops.get();
            started_ops.set(current_op + 1);
            let progress_per_op = 100.0 / (total_ops.get().max(started_ops.get()) as f32);
            log::info!(
                "Operation {}: {:?} {:?}",
                current_op,
                op.operation_type(),
                op.get_ref()
            );
            let callback = callback.clone();
            progress.connect_changed(move |progress| {
                log::info!(
                    "{}: {}%",
                    progress.status().unwrap_or_default(),
                    progress.progress()
                );
                let op_progress = (progress.progress() as f32) / 100.0;
                let total_progress = ((current_op as f32) + op_progress) * progress_per_op;
                let mut callback = callback.lock().unwrap();
                callback(total_progress)
            });
        });
        match op.kind {
            OperationKind::Install => {
                for info in op.infos.iter() {
                    for r_str in info.flatpak_refs.iter() {
                        let r = match Ref::parse(r_str) {
                            Ok(ok) => ok,
                            Err(err) => {
                                log::warn!("failed to parse flatpak ref {:?}: {}", r_str, err);
                                continue;
                            }
                        };
                        for remote in inst.list_remotes(Cancellable::NONE)? {
                            let Some(remote_name) = remote.name() else {
                                continue;
                            };
                            if remote_name != info.source_id {
                                continue;
                            }
                            match inst.fetch_remote_ref_sync(
                                &remote_name,
                                r.kind(),
                                &r.name().unwrap_or_default(),
                                r.arch().as_deref(),
                                r.branch().as_deref(),
                                Cancellable::NONE,
                            ) {
                                Ok(_) => {}
                                Err(err) => {
                                    log::info!(
                                        "failed to find {} in {}: {}",
                                        r_str,
                                        remote_name,
                                        err
                                    );
                                    continue;
                                }
                            };

                            log::info!("installing flatpak {} from remote {}", r_str, remote_name);
                            tx.add_install(&remote_name, &r_str, &[])?;
                            //TODO: install all refs?
                            break;
                        }
                    }
                }
            }
            OperationKind::Uninstall => {
                //TODO: deduplicate code
                for info in op.infos.iter() {
                    for r_str in info.flatpak_refs.iter() {
                        let r = match Ref::parse(r_str) {
                            Ok(ok) => ok,
                            Err(err) => {
                                log::warn!("failed to parse flatpak ref {}: {}", r_str, err);
                                continue;
                            }
                        };
                        match inst.installed_ref(
                            r.kind(),
                            &r.name().unwrap_or_default(),
                            r.arch().as_deref(),
                            r.branch().as_deref(),
                            Cancellable::NONE,
                        ) {
                            Ok(_) => {}
                            Err(err) => {
                                log::info!("failed to find {} installed locally: {}", r_str, err);
                                continue;
                            }
                        };

                        log::info!("uninstalling flatpak {}", r_str);
                        tx.add_uninstall(&r_str)?;
                    }
                }
            }
            OperationKind::Update => {
                //TODO: deduplicate code
                for info in op.infos.iter() {
                    for r_str in info.flatpak_refs.iter() {
                        let r = match Ref::parse(r_str) {
                            Ok(ok) => ok,
                            Err(err) => {
                                log::warn!("failed to parse flatpak ref {}: {}", r_str, err);
                                continue;
                            }
                        };
                        match inst.installed_ref(
                            r.kind(),
                            &r.name().unwrap_or_default(),
                            r.arch().as_deref(),
                            r.branch().as_deref(),
                            Cancellable::NONE,
                        ) {
                            Ok(_) => {}
                            Err(err) => {
                                log::info!("failed to find {} installed locally: {}", r_str, err);
                                continue;
                            }
                        };

                        log::info!("updating flatpak {}", r_str);
                        tx.add_update(&r_str, &[], None)?;
                    }
                }
            }
        }
        tx.run(Cancellable::NONE)?;
        Ok(())
    }
}
