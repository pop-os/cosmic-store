use cosmic::widget;
use libflatpak::{gio::Cancellable, prelude::*, Installation, RefKind};
use std::error::Error;

use super::{Backend, Package};

pub struct Flatpak {
    inst: Installation,
}

impl Flatpak {
    pub fn new() -> Result<Self, Box<dyn Error>> {
        //TODO: should we support system installations?
        let inst = Installation::new_user(Cancellable::NONE)?;
        println!("{:?}", (inst.id(), inst.display_name()));
        Ok(Self { inst })
    }
}

impl Backend for Flatpak {
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let installed = self
            .inst
            .list_installed_refs_by_kind(RefKind::App, Cancellable::NONE)?;
        let mut packages = Vec::new();
        for r in installed.iter() {
            if let Some(id) = r.name() {
                packages.push(Package {
                    id: id.to_string(),
                    icon: widget::icon::from_name(id.to_string()),
                    name: r.appdata_name().unwrap_or(id).to_string(),
                    version: r.appdata_version().unwrap_or_default().to_string(),
                })
            }
        }
        Ok(packages)
    }
}
