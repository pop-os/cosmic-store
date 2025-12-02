use cosmic::widget;
use libflatpak::{Installation, Ref, Remote, Transaction, gio::Cancellable, glib, prelude::*};
use std::{
    cell::{Cell, RefCell},
    collections::HashMap,
    error::Error,
    fmt::Write,
    fs,
    rc::Rc,
    sync::Arc,
};

use super::{Backend, Package};
use crate::{
    AppId, AppInfo, AppUrl, AppstreamCache, Operation, OperationKind, RepositoryRemoveError,
};

#[derive(Debug)]
pub struct Flatpak {
    user: bool,
    appstream_caches: Vec<AppstreamCache>,
}

impl Flatpak {
    fn installation(&self) -> Result<Installation, glib::Error> {
        if self.user {
            Installation::new_user(Cancellable::NONE)
        } else {
            Installation::new_system(Cancellable::NONE)
        }
    }

    fn source_id(&self, remote_name: &str) -> String {
        if self.user {
            remote_name.to_string()
        } else {
            format!("{remote_name} (system)")
        }
    }

    pub fn new(user: bool, locale: &str) -> Result<Self, Box<dyn Error>> {
        let mut this = Self {
            user,
            appstream_caches: Vec::new(),
        };

        let inst = this.installation()?;
        for remote in inst.list_remotes(Cancellable::NONE)? {
            let source_id = match remote.name() {
                Some(name) => this.source_id(&name),
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
                Some(title) => this.source_id(&title),
                None => source_id.clone(),
            };
            this.appstream_caches.push(AppstreamCache::new(
                source_id,
                source_name,
                paths,
                icons_paths,
                locale,
            ));
        }

        // We don't store the installation because it is not Send
        Ok(this)
    }

    fn ref_to_package<R: InstalledRefExt + RefExt>(&self, r: &R) -> Option<Package> {
        let id_raw = r.name()?;
        let id = AppId::new(&id_raw);
        let origin = r.origin()?;
        for appstream_cache in self.appstream_caches.iter() {
            if appstream_cache.source_id != self.source_id(&origin) {
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
                    name,
                    summary,
                    description,
                    flatpak_refs,
                    ..Default::default()
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
            let inst = self.installation()?;
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
        let inst = self.installation()?;
        let packages = self.refs_to_packages(inst.list_installed_refs(Cancellable::NONE)?);
        Ok(packages)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let inst = self.installation()?;
        let packages =
            self.refs_to_packages(inst.list_installed_refs_for_update(Cancellable::NONE)?);
        Ok(packages)
    }

    fn file_packages(&self, path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        if !self.user {
            return Err(
                "flatpak backend only supports installing files with user installation"
                    .to_string()
                    .into(),
            );
        }

        if !path.ends_with(".flatpakref") {
            return Err(format!("flatpak backend does not support file {path:?}").into());
        }

        let entry = freedesktop_entry_parser::parse_entry(path)?;
        if !entry.has_section("Flatpak Ref") {
            return Err(format!("flatpak ref {path:?} missing Flatpak Ref section").into());
        }

        let get_attr = |key| entry.get("Flatpak Ref", key).and_then(|attr| attr.first());

        let id = get_attr("Name")
            .ok_or_else(|| format!("flatpak ref {path:?} missing Name attribute"))?;
        let url =
            get_attr("Url").ok_or_else(|| format!("flatpak ref {path:?} missing Url attribute"))?;

        let mut source_id = url.to_string();
        let mut source_name = url.to_string();
        let inst = self.installation()?;
        for remote in inst.list_remotes(Cancellable::NONE)? {
            if remote.url().is_some_and(|u| u == *url) {
                // Check if already installed
                if let Ok(r) = inst.current_installed_app(id, Cancellable::NONE) {
                    return Ok(self.refs_to_packages(vec![r]));
                }
                let Some(name) = remote.name() else {
                    log::warn!("remote {:?} missing name", remote);
                    continue;
                };

                source_id = self.source_id(&name);
                source_name = remote
                    .title()
                    .map(|t| self.source_id(&t))
                    .unwrap_or(source_id.clone());

                break;
            }
        }

        let mut extra = HashMap::new();
        if let Some(branch) = get_attr("Branch") {
            extra.insert("branch".to_string(), branch.to_string());
        }

        Ok(vec![Package {
            id: AppId::new(id),
            icon: widget::icon::from_name("package-x-generic")
                .size(128)
                .handle(),
            //TODO: fill in more AppInfo fields
            info: Arc::new(AppInfo {
                source_id,
                source_name,
                name: get_attr("Title").unwrap_or(id).to_string(),
                summary: get_attr("Comment").cloned().unwrap_or_default(),
                description: get_attr("Description").cloned().unwrap_or_default(),
                urls: get_attr("Homepage")
                    .map(|h| vec![AppUrl::Homepage(h.to_string())])
                    .unwrap_or_default(),
                package_paths: vec![path.to_string()],
                ..Default::default()
            }),
            version: String::new(),
            extra,
        }])
    }

    fn operation(
        &self,
        op: &Operation,
        callback: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        let callback = Rc::new(RefCell::new(callback));
        let inst = self.installation()?;
        let total_ops = Rc::new(Cell::new(0));
        let tx = Transaction::for_installation(&inst, Cancellable::NONE)?;
        {
            let total_ops = total_ops.clone();
            tx.connect_ready(move |tx| {
                total_ops.set(tx.operations().len());
                true
            });
        }
        let started_ops = Rc::new(Cell::new(0));
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
                let mut callback = callback.borrow_mut();
                callback(total_progress)
            });
        });
        match &op.kind {
            OperationKind::Install => {
                for info in op.infos.iter() {
                    if !info.package_paths.is_empty() {
                        for package_path in info.package_paths.iter() {
                            log::info!("installing flatpak ref {:?}", package_path);
                            //TODO: keep package data in memory?
                            let data = fs::read(package_path)?;
                            let bytes = glib::Bytes::from_owned(data);
                            tx.add_install_flatpakref(&bytes)?;
                        }
                    } else {
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
                                if self.source_id(&remote_name) != info.source_id {
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

                                log::info!(
                                    "installing flatpak {} from remote {}",
                                    r_str,
                                    remote_name
                                );
                                tx.add_install(&remote_name, r_str, &[])?;
                                //TODO: install all refs?
                                break;
                            }
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
                        tx.add_uninstall(r_str)?;
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
                        tx.add_update(r_str, &[], None)?;
                    }
                }
            }
            OperationKind::RepositoryAdd(adds) => {
                drop(tx);
                let mut remotes = Vec::with_capacity(adds.len());
                for add in adds.iter() {
                    remotes.push(Remote::from_file(&add.id, &glib::Bytes::from(&add.data))?);
                }
                for remote in remotes {
                    inst.add_remote(&remote, true, Cancellable::NONE)?;
                }
                return Ok(());
            }
            OperationKind::RepositoryRemove(rms, force) => {
                let mut installed = Vec::new();
                for r in inst.list_installed_refs(Cancellable::NONE)? {
                    let Some(origin) = r.origin() else {
                        continue;
                    };
                    if !rms.iter().any(|rm| rm.id == origin) {
                        continue;
                    }
                    if *force {
                        let Some(ref_str) = r.format_ref() else {
                            continue;
                        };
                        tx.add_uninstall(ref_str.as_str())?;
                    } else {
                        let Some(name) = r.name() else { continue };
                        let appdata_name = r.appdata_name().unwrap_or_else(|| name.clone());
                        installed.push((name.to_string(), appdata_name.to_string()));
                    }
                }
                if !installed.is_empty() {
                    installed.sort_by(|a, b| crate::LANGUAGE_SORTER.compare(&a.1, &b.1));
                    return Err(RepositoryRemoveError {
                        rms: rms.clone(),
                        installed,
                    }
                    .into());
                }
                if *force {
                    tx.run(Cancellable::NONE)?;
                }
                drop(tx);
                for rm in rms.iter() {
                    inst.remove_remote(&rm.id, Cancellable::NONE)?;
                }
                return Ok(());
            }
        }
        tx.run(Cancellable::NONE)?;
        Ok(())
    }
}
