use cosmic::widget;
use packagekit_zbus::{
    PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
    zbus::{blocking::Connection, zvariant},
};
use std::{collections::HashMap, error::Error, fmt::Write, sync::Arc};

use super::{Backend, Package};
use crate::{AppId, AppInfo, AppUrl, AppstreamCache, GStreamerCodec, Operation, OperationKind};

#[derive(Debug)]
struct TransactionDetails {
    //TODO: more fields: https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::Details
    package_id: String,
    summary: String,
    description: String,
    url: String,
}

#[allow(dead_code)]
#[derive(Debug)]
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
    mut on_progress: impl FnMut(u32, TransactionProgress),
) -> Result<(Vec<TransactionDetails>, Vec<TransactionPackage>), Box<dyn Error>> {
    let mut details = Vec::new();
    let mut packages = Vec::new();
    for signal in tx.receive_all_signals()? {
        if let Some(member) = signal.member() {
            match member.as_str() {
                "Details" => {
                    let map = signal.body::<HashMap<String, zvariant::Value>>()?;

                    let get_string = |key: &str| -> Option<String> {
                        match map.get(key) {
                            Some(zvariant::Value::Str(str)) => Some(str.to_string()),
                            unknown => {
                                log::warn!(
                                    "failed to find string for key {:?} in packagekit Details: found {:?} instead",
                                    key,
                                    unknown
                                );
                                None
                            }
                        }
                    };

                    let Some(package_id) = get_string("package-id") else {
                        continue;
                    };
                    let summary = get_string("summary").unwrap_or_default();
                    let description = get_string("description").unwrap_or_default();
                    let url = get_string("url").unwrap_or_default();
                    details.push(TransactionDetails {
                        package_id,
                        summary,
                        description,
                        url,
                    });
                }
                "ErrorCode" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::ErrorCode
                    let (code, details) = signal.body::<(u32, String)>()?;
                    if code != 48 {
                        return Err(format!("{details} (code {code})").into());
                    }
                }
                "ItemProgress" => {
                    // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::ItemProgress
                    let (package_id, status, percentage) = signal.body::<(String, u32, u32)>()?;
                    let total_percentage = tx.percentage().unwrap_or(percentage);
                    on_progress(
                        total_percentage,
                        TransactionProgress {
                            package_id,
                            status,
                            percentage,
                        },
                    )
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
            }
        }
    }
    Ok((details, packages))
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

#[allow(dead_code)]
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
    appstream_caches: Vec<AppstreamCache>,
}

impl Packagekit {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        //TODO: cache more zbus stuff?
        let connection = Connection::system()?;
        let source_id = "packagekit";
        //TODO: translate?
        let source_name = "System";
        Ok(Self {
            connection,
            appstream_caches: vec![AppstreamCache::system(
                source_id.to_string(),
                source_name.to_string(),
                locale,
            )],
        })
    }

    fn transaction(&self) -> Result<TransactionProxyBlocking<'_>, Box<dyn Error>> {
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
        let appstream_cache = &self.appstream_caches[0];

        let (tx_details, tx_packages) = transaction_handle(tx, |_, _| {})?;

        let mut system_packages = Vec::new();
        let mut packages = Vec::new();

        for tx_detail in tx_details {
            //TODO: this is a hack to handle file details like they are packages
            let mut parts = tx_detail.package_id.split(';');
            let Some(package_name) = parts.next() else {
                continue;
            };
            let version_opt = parts.next();
            let _architecture_opt = parts.next();

            let data = parts.next().unwrap_or("");
            let mut data_parts = data.split(':');
            let _status_opt = data_parts.next();
            let _origin_opt = data_parts.next();

            //TODO: translate
            packages.push(Package {
                id: AppId::new(package_name),
                icon: widget::icon::from_name("package-x-generic")
                    .size(128)
                    .handle(),
                //TODO: fill in more AppInfo fields
                info: Arc::new(AppInfo {
                    source_id: appstream_cache.source_id.clone(),
                    source_name: appstream_cache.source_name.clone(),
                    name: package_name.to_string(),
                    summary: tx_detail.summary.clone(),
                    description: tx_detail.description.clone(),
                    pkgnames: vec![package_name.to_string()],
                    urls: if !tx_detail.url.is_empty() {
                        vec![AppUrl::Homepage(tx_detail.url.to_string())]
                    } else {
                        Vec::new()
                    },
                    ..Default::default()
                }),
                version: version_opt.unwrap_or("").to_string(),
                extra: HashMap::new(),
            });
        }

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

            match appstream_cache.pkgnames.get(package_name) {
                Some(ids) => {
                    for id in ids.iter() {
                        match appstream_cache.infos.get(id) {
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
}

impl Backend for Packagekit {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>> {
        if refresh {
            let tx = self.transaction()?;
            tx.set_hints(&["interactive=true", "cache-age=300"])?;
            tx.refresh_cache(false)?;
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
        let tx = self.transaction()?;
        tx.get_packages(FilterKind::Installed as u64)?;
        self.package_transaction(tx)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let tx = self.transaction()?;
        tx.get_updates(FilterKind::None as u64)?;
        self.package_transaction(tx)
    }

    fn file_packages(&self, path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        let tx = self.transaction()?;
        tx.get_details_local(&[path])?;
        let mut packages = self.package_transaction(tx)?;
        for package in packages.iter_mut() {
            let info = Arc::make_mut(&mut package.info);
            info.package_paths.push(path.to_string());
        }
        Ok(packages)
    }

    fn gstreamer_packages(
        &self,
        gstreamer_codec: &GStreamerCodec,
    ) -> Result<Vec<Package>, Box<dyn Error>> {
        // Packagekit provides looks like gstreamer1.0(decoder-video/x-h264)
        //TODO: truncate version ending in .0? gstreamer1.0-packagekit does this but it does not appear to be required
        let provides = format!(
            "gstreamer{}({})",
            gstreamer_codec.version, gstreamer_codec.type_name
        );
        let tx = self.transaction()?;
        tx.what_provides(
            FilterKind::Newest as u64 | FilterKind::Arch as u64,
            &[&provides],
        )?;
        let (_tx_details, tx_packages) = transaction_handle(tx, |_, _| {})?;

        // Convert packages to details in order to show more information
        let mut package_ids = Vec::with_capacity(tx_packages.len());
        for tx_package in tx_packages.iter() {
            package_ids.push(tx_package.package_id.as_str());
        }
        let tx = self.transaction()?;
        tx.get_details(&package_ids)?;
        self.package_transaction(tx)
    }

    fn operation(
        &self,
        op: &Operation,
        mut f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        let mut package_names = Vec::new();
        let mut package_paths = Vec::new();
        for info in op.infos.iter() {
            for pkgname in &info.pkgnames {
                package_names.push(pkgname.as_str());
            }
            for package_path in &info.package_paths {
                package_paths.push(package_path.as_str());
            }
        }
        if package_names.is_empty() {
            return Err(format!("{:?} missing package name", op.package_ids).into());
        }
        let (_tx_details, tx_packages) = {
            let tx = self.transaction()?;
            log::info!("resolve packages for {:?}", package_names);
            let filter = match &op.kind {
                OperationKind::Install | OperationKind::Update => {
                    FilterKind::NotInstalled as u64
                        | FilterKind::Newest as u64
                        | FilterKind::Arch as u64
                }
                OperationKind::Uninstall { .. } => FilterKind::Installed as u64,
                // Other operations not supported
                _ => 0,
            };
            tx.resolve(filter, &package_names)?;
            transaction_handle(tx, |_, _| {})?
        };
        let mut package_ids = Vec::with_capacity(package_names.len());
        for tx_package in tx_packages.iter() {
            package_ids.push(tx_package.package_id.as_str());
        }
        let tx = self.transaction()?;
        tx.set_hints(&["interactive=true"])?;
        match &op.kind {
            OperationKind::Install => {
                if !package_paths.is_empty() {
                    log::info!("installing package files {:?}", package_paths);
                    //TODO: transaction flags
                    tx.install_files(0, &package_paths)?;
                } else {
                    log::info!("installing packages {:?}", package_ids);
                    //TODO: transaction flags
                    tx.install_packages(TransactionFlag::OnlyTrusted as u64, &package_ids)?;
                }
            }
            OperationKind::Uninstall { purge_data } => {
                log::info!("uninstalling packages {:?} (purge_data: {})", package_ids, purge_data);
                if *purge_data {
                    log::warn!(
                        "PackageKit backend does not fully support purging configuration files. \
                        Only the package will be removed. Configuration files may remain in user directories."
                    );
                }
                //TODO: transaction flags?
                //TODO: investigate if we can detect package managers like dnf, apt, etc
                // and use purge-specific functionality
                tx.remove_packages(0, &package_ids, true, true)?;
            }
            OperationKind::Update => {
                log::info!("updating packages {:?}", package_ids);
                //TODO: transaction flags?
                tx.update_packages(TransactionFlag::OnlyTrusted as u64, &package_ids)?;
            }
            OperationKind::RepositoryAdd { .. } => {
                return Err("packagekit backend does not support adding repositories".into());
            }
            OperationKind::RepositoryRemove { .. } => {
                return Err("packagekit backend does not support removing repositories".into());
            }
        }
        let _tx_packages = transaction_handle(tx, |total_percentage, progress| {
            log::info!(
                "{}%: {} {} {}%",
                total_percentage,
                progress.package_id,
                progress.status,
                progress.percentage
            );
            f(total_percentage as f32);
        })?;
        Ok(())
    }
}
