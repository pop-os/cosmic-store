use cosmic::widget;
use packagekit_zbus::{
    zbus::blocking::Connection, PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
};
use std::{collections::HashMap, error::Error, fmt::Write, sync::Arc};

use super::{Backend, Package};
use crate::{AppInfo, AppstreamCache, OperationKind, SYSTEM_ID};

struct TransactionPackage {
    info: u32,
    package_id: String,
    summary: String,
}

struct TransactionProgress {
    package_id: String,
    status: u32,
    percentage: u32,
}

fn transaction_handle(
    tx: TransactionProxyBlocking,
    mut on_progress: impl FnMut(TransactionProgress),
) -> Result<Vec<TransactionPackage>, Box<dyn Error>> {
    let mut packages = Vec::new();
    for signal in tx.receive_all_signals()? {
        match signal.member() {
            Some(member) => match member.as_str() {
                "ErrorCode" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::ErrorCode
                    let (code, details) = signal.body::<(u32, String)>()?;
                    return Err(format!("{details} (code {code})").into());
                }
                "ItemProgress" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::ItemProgress
                    let (package_id, status, percentage) = signal.body::<(String, u32, u32)>()?;
                    on_progress(TransactionProgress {
                        package_id,
                        status,
                        percentage,
                    })
                }
                "Package" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::Package
                    let (info, package_id, summary) = signal.body::<(u32, String, String)>()?;
                    packages.push(TransactionPackage {
                        info,
                        package_id,
                        summary,
                    });
                }
                "Finished" => {
                    break;
                }
                _ => {
                    log::warn!("unknown signal {}", member);
                }
            },
            None => {}
        }
    }
    Ok(packages)
}

// https://lazka.github.io/pgi-docs/PackageKitGlib-1.0/enums.html#PackageKitGlib.FilterEnum
#[repr(u64)]
enum FilterKind {
    None = 1 << 1,
    Installed = 1 << 2,
    NotInstalled = 1 << 3,
    Newest = 1 << 16,
    Arch = 1 << 18,
}

#[repr(u64)]
enum TransactionFlag {
    None = 1 << 0,
    OnlyTrusted = 1 << 1,
    AllowReinstall = 1 << 4,
    AllowDowngrade = 1 << 6,
}

#[derive(Debug)]
pub struct Packagekit {
    connection: Connection,
    appstream_cache: AppstreamCache,
}

impl Packagekit {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        //TODO: cache more zbus stuff?
        let connection = Connection::system()?;
        Ok(Self {
            connection,
            appstream_cache: AppstreamCache::system(locale),
        })
    }

    fn transaction(&self) -> Result<TransactionProxyBlocking, Box<dyn Error>> {
        //TODO: use async?
        let pk = PackageKitProxyBlocking::new(&self.connection)?;
        //TODO: set locale?
        let tx_path = pk.create_transaction()?;
        let tx = TransactionProxyBlocking::builder(&self.connection)
            .destination("org.freedesktop.PackageKit")?
            .path(tx_path)?
            .build()?;
        Ok(tx)
    }

    fn package_transaction(
        &self,
        tx: TransactionProxyBlocking,
    ) -> Result<Vec<Package>, Box<dyn Error>> {
        let tx_packages = transaction_handle(tx, |_| {})?;

        let mut system_packages = Vec::new();
        let mut packages = Vec::new();
        for tx_package in tx_packages {
            let mut parts = tx_package.package_id.split(';');
            let Some(package_name) = parts.next() else {
                continue;
            };
            let version_opt = parts.next();
            let _architecture_opt = parts.next();

            let data = parts.next().unwrap_or("");
            let mut data_parts = data.split(':');
            let _status_opt = data_parts.next();
            let _origin_opt = data_parts.next();

            match self.appstream_cache.pkgnames.get(package_name) {
                Some(ids) => {
                    for id in ids.iter() {
                        match self.appstream_cache.infos.get(id) {
                            Some(info) => {
                                packages.push(Package {
                                    id: id.clone(),
                                    icon: self.appstream_cache.icon(info),
                                    info: info.clone(),
                                    version: version_opt.unwrap_or("").to_string(),
                                    extra: HashMap::new(),
                                });
                            }
                            None => {
                                log::warn!("failed to find info {}", id);
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
        if !system_packages.is_empty() {
            let name = "System Packages".to_string();
            let summary = format!(
                "{} package{}",
                system_packages.len(),
                if system_packages.len() == 1 { "" } else { "s" }
            );
            let mut description = String::new();
            let mut pkgnames = Vec::with_capacity(system_packages.len());
            for (package_name, version) in system_packages {
                let _ = writeln!(description, " * {}: {}", package_name, version);
                pkgnames.push(package_name);
            }
            //TODO: translate
            packages.push(Package {
                id: SYSTEM_ID.to_string(),
                icon: widget::icon::from_name("package-x-generic")
                    .size(128)
                    .handle(),
                //TODO: fill in more AppInfo fields
                info: Arc::new(AppInfo {
                    origin_opt: None,
                    name,
                    summary,
                    description,
                    pkgnames,
                    categories: Vec::new(),
                    desktop_ids: Vec::new(),
                    flatpak_refs: Vec::new(),
                    icons: Vec::new(),
                    screenshots: Vec::new(),
                }),
                version: String::new(),
                extra: HashMap::new(),
            });
        }
        Ok(packages)
    }
}

impl Backend for Packagekit {
    fn load_cache(&mut self) -> Result<(), Box<dyn Error>> {
        self.appstream_cache.reload("packagekit");
        Ok(())
    }

    fn info_cache(&self) -> &AppstreamCache {
        &self.appstream_cache
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let tx = self.transaction()?;
        tx.get_packages(FilterKind::Installed as u64)?;
        self.package_transaction(tx)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let tx = self.transaction()?;
        tx.get_updates(FilterKind::None as u64)?;
        self.package_transaction(tx)
    }

    fn operation(
        &self,
        kind: OperationKind,
        package_id: &str,
        info: &AppInfo,
        mut f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        let mut package_names = Vec::with_capacity(info.pkgnames.len());
        for pkgname in &info.pkgnames {
            package_names.push(pkgname.as_str());
        }
        if package_names.is_empty() {
            return Err(format!("{} missing package name", package_id).into());
        }
        let tx_packages = {
            let tx = self.transaction()?;
            log::info!("resolve packages for {:?}", package_names);
            let filter = match kind {
                OperationKind::Install | OperationKind::Update => {
                    FilterKind::NotInstalled as u64
                        | FilterKind::Newest as u64
                        | FilterKind::Arch as u64
                }
                OperationKind::Uninstall => FilterKind::Installed as u64,
            };
            tx.resolve(filter, &package_names)?;
            transaction_handle(tx, |_| {})?
        };
        let mut package_ids = Vec::with_capacity(package_names.len());
        for tx_package in tx_packages.iter() {
            package_ids.push(tx_package.package_id.as_str());
        }
        let tx = self.transaction()?;
        tx.set_hints(&["interactive=true"])?;
        match kind {
            OperationKind::Install => {
                log::info!("installing packages {:?}", package_ids);
                //TODO: transaction flags
                tx.install_packages(TransactionFlag::OnlyTrusted as u64, &package_ids)?;
            }
            OperationKind::Uninstall => {
                log::info!("uninstalling packages {:?}", package_ids);
                //TODO: transaction flags?
                tx.remove_packages(0, &package_ids, true, true)?;
            }
            OperationKind::Update => {
                log::info!("updating packages {:?}", package_ids);
                //TODO: transaction flags?
                tx.update_packages(TransactionFlag::OnlyTrusted as u64, &package_ids)?;
            }
        }
        let _tx_packages = transaction_handle(tx, |progress| {
            log::info!(
                "{} {} {}%",
                progress.package_id,
                progress.status,
                progress.percentage
            );
            //TODO: show progress as total of all items
            f(progress.percentage as f32);
        })?;
        Ok(())
    }
}
