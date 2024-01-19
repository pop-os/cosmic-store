use appstream::Collection;
use cosmic::widget;
use std::{collections::HashMap, error::Error};

#[cfg(feature = "flatpak")]
mod flatpak;

mod system;

#[derive(Clone, Debug)]
pub struct Package {
    pub id: String,
    pub icon: widget::icon::Named,
    pub name: String,
    pub version: String,
    pub extra: HashMap<String, String>,
}

pub trait Backend {
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn appstream(&self, package: &Package) -> Result<Collection, Box<dyn Error>>;
}

pub fn backends() -> Vec<Box<dyn Backend>> {
    let mut backends = Vec::<Box<dyn Backend>>::new();

    match system::System::new() {
        Ok(backend) => {
            backends.push(Box::new(backend));
        }
        Err(err) => {
            log::error!("failed to load system backend: {}", err);
        }
    }

    #[cfg(feature = "flatpak")]
    {
        match flatpak::Flatpak::new() {
            Ok(backend) => {
                backends.push(Box::new(backend));
            }
            Err(err) => {
                log::error!("failed to load flatpak backend: {}", err);
            }
        }
    }

    backends
}
