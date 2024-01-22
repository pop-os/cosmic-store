use packagekit_zbus::{
    zbus::blocking::Connection, PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
};
use std::{collections::HashMap, error::Error, sync::Arc};

use super::{Backend, Package};
use crate::AppstreamCache;

// https://lazka.github.io/pgi-docs/PackageKitGlib-1.0/enums.html#PackageKitGlib.FilterEnum
#[repr(u64)]
enum FilterKind {
    Installed = 1 << 2,
}

#[derive(Debug)]
pub struct Packagekit {
    connection: Connection,
    appstream_cache: Arc<AppstreamCache>,
}

impl Packagekit {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        //TODO: cache more zbus stuff?
        let connection = Connection::system()?;
        Ok(Self {
            connection,
            appstream_cache: Arc::new(AppstreamCache::system(locale)),
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

    fn packages(&self, filter: FilterKind) -> Result<Vec<Package>, Box<dyn Error>> {
        let mut package_ids = Vec::new();
        {
            let tx = self.transaction()?;
            tx.get_packages(filter as u64)?;
            for signal in tx.receive_all_signals()? {
                match signal.member() {
                    Some(member) => {
                        if member == "Package" {
                            // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::Package
                            let (_info, package_id, _summary) =
                                signal.body::<(u32, String, String)>()?;
                            package_ids.push(package_id);
                        } else if member == "Finished" {
                            break;
                        } else {
                            log::warn!("unknown signal {}", member);
                        }
                    }
                    None => {}
                }
            }
        }

        let mut packages = Vec::new();
        for package_id in package_ids {
            let mut parts = package_id.split(';');
            let package_name = parts.next().unwrap_or(&package_id);
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
                                    name: info.name.clone(),
                                    summary: info.summary.clone(),
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
                }
            }
        }
        Ok(packages)
    }
}

impl Backend for Packagekit {
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        self.packages(FilterKind::Installed)
    }

    fn info_cache(&self) -> &Arc<AppstreamCache> {
        &self.appstream_cache
    }
}
