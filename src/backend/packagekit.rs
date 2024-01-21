use appstream::Collection;
use cosmic::widget;
use packagekit_zbus::{
    zbus::blocking::Connection, PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
};
use std::{collections::HashMap, error::Error, sync::Arc};

use super::{Backend, Package};
use crate::{get_translatable, AppstreamCache};

pub struct Packagekit {
    connection: Connection,
    appstream_cache: Arc<AppstreamCache>,
    locale: String,
}

impl Packagekit {
    pub fn new(
        appstream_cache: &Arc<AppstreamCache>,
        locale: &str,
    ) -> Result<Self, Box<dyn Error>> {
        //TODO: cache more zbus stuff?
        let connection = Connection::system()?;
        Ok(Self {
            connection,
            appstream_cache: appstream_cache.clone(),
            locale: locale.to_string(),
        })
    }

    fn transaction(&self) -> Result<TransactionProxyBlocking, Box<dyn Error>> {
        //TODO: use async?
        let pk = PackageKitProxyBlocking::new(&self.connection)?;
        //TODO: set locale
        let tx_path = pk.create_transaction()?;
        let tx = TransactionProxyBlocking::builder(&self.connection)
            .destination("org.freedesktop.PackageKit")?
            .path(tx_path)?
            .build()?;
        Ok(tx)
    }
}

impl Backend for Packagekit {
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let mut package_ids = Vec::new();
        {
            let tx = self.transaction()?;
            let filter_installed = 1 << 2;
            //let filter_application = 1 << 24;
            tx.get_packages(filter_installed)?;
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
                            println!("unknown signal {}", member);
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
            let architecture_opt = parts.next();

            let data = parts.next().unwrap_or("");
            let mut data_parts = data.split(':');
            let status_opt = data_parts.next();
            let origin_opt = data_parts.next();

            let mut extra = HashMap::new();
            if let Some(architecture) = architecture_opt {
                extra.insert("architecture".to_string(), architecture.to_string());
            }
            if let Some(origin) = origin_opt {
                extra.insert("origin".to_string(), origin.to_string());
            }
            match self.appstream_cache.pkgnames.get(package_name) {
                Some(component_ids) => {
                    for component_id in component_ids.iter() {
                        match self.appstream_cache.components.get(component_id) {
                            Some(component) => {
                                let mut icon_name = "package-x-generic".to_string();
                                for icon in component.icons.iter() {
                                    //TODO: support other types of icons
                                    match icon {
                                        appstream::enums::Icon::Stock(stock) => {
                                            icon_name = stock.clone();
                                        }
                                        _ => {}
                                    }
                                }
                                packages.push(Package {
                                    id: component_id.clone(),
                                    //TODO: get icon from appstream data?
                                    icon: widget::icon::from_name(icon_name),
                                    name: get_translatable(&component.name, &self.locale)
                                        .to_string(),
                                    version: version_opt.unwrap_or("").to_string(),
                                    extra: extra.clone(),
                                });
                            }
                            None => {
                                log::warn!("failed to find component {}", component_id);
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

    fn appstream(&self, package: &Package) -> Result<Collection, Box<dyn Error>> {
        match self.appstream_cache.components.get(&package.id) {
            Some(component) => {
                Ok(Collection {
                    //TODO: fill in this field
                    version: String::new(),
                    origin: package.extra.get("origin").cloned(),
                    components: vec![component.clone()],
                    architecture: package.extra.get("architecture").cloned(),
                })
            }
            None => Err(format!("failed to find component {}", package.id).into()),
        }
    }
}
