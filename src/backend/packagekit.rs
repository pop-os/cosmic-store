use appstream::Collection;
use packagekit_zbus::{
    zbus::blocking::Connection, PackageKit::PackageKitProxyBlocking,
    Transaction::TransactionProxyBlocking,
};
use std::error::Error;

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
}

impl Backend for Packagekit {
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        //TODO: use async?
        let pk = PackageKitProxyBlocking::new(&self.connection)?;
        let tx_path = pk.create_transaction()?;
        let tx = TransactionProxyBlocking::builder(&self.connection)
            .destination("org.freedesktop.PackageKit")?
            .path(&tx_path)?
            .build()?;
        let filter_installed = 1 << 2;
        tx.get_packages(filter_installed)?;
        for signal in tx.receive_all_signals()? {
            match signal.member() {
                Some(member) => {
                    if member == "Package" {
                        // https://www.freedesktop.org/software/PackageKit/gtk-doc/Transaction.html#Transaction::Package
                        let (info, package_id, summary) = signal.body::<(u32, &str, &str)>()?;
                        println!("{} {}: {}", info, package_id, summary);
                    } else if member == "Finished" {
                        break;
                    }
                }
                None => {}
            }
        }
        //TODO
        //let packages = tx.receive_packages()?;
        //println!("{:?}", packages);
        Err("unimplemented".into())
    }

    fn appstream(&self, package: &Package) -> Result<Collection, Box<dyn Error>> {
        Err("unimplemented".into())
    }
}
