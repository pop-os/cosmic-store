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
        Err("unimplemented".into())
    }

    fn appstream(&self, package: &Package) -> Result<Collection, Box<dyn Error>> {
        Err("unimplemented".into())
    }
}
