use appstream::Collection;
use cosmic::widget;
use packagekit_zbus::{
    zbus::blocking::Connection, PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
};
use std::{collections::HashMap, error::Error};

use super::{Backend, Package};

pub struct Packagekit {
    connection: Connection,
}

impl Packagekit {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        //TODO: cache more zbus stuff?
        let connection = Connection::system()?;
        Ok(Self { connection })
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
            let filter_application = 1 << 24;
            tx.get_packages(filter_installed | filter_application)?;
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
            let name = parts.next().unwrap_or(&package_id).to_string();
            let version = parts.next().unwrap_or_default().to_string();
            packages.push(Package {
                id: package_id,
                //TODO: get icon from appstream data?
                icon: widget::icon::from_name("package-x-generic"),
                name,
                version,
                extra: HashMap::new(),
            });
        }
        Ok(packages)
    }

    fn appstream(&self, package: &Package) -> Result<Collection, Box<dyn Error>> {
        Err("unimplemented".into())
    }
}
