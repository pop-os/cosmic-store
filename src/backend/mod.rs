use appstream::Collection;
use cosmic::widget;
use std::{collections::HashMap, error::Error, fmt, sync::Arc};

use crate::AppstreamCache;

#[cfg(feature = "flatpak")]
mod flatpak;

#[cfg(feature = "packagekit")]
mod packagekit;

#[derive(Clone, Debug)]
pub struct Package {
    pub id: String,
    pub icon: widget::icon::Handle,
    pub name: String,
    pub summary: String,
    pub version: String,
    pub extra: HashMap<String, String>,
}

pub trait Backend: fmt::Debug + Send + Sync {
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn appstream(&self, package: &Package) -> Result<Arc<Collection>, Box<dyn Error>>;
}

pub type Backends = HashMap<&'static str, Arc<dyn Backend>>;

pub fn backends(appstream_cache: &Arc<AppstreamCache>, locale: &str) -> Backends {
    let mut backends = Backends::new();

    #[cfg(feature = "flatpak")]
    {
        match flatpak::Flatpak::new() {
            Ok(backend) => {
                backends.insert("flatpak", Arc::new(backend));
            }
            Err(err) => {
                log::error!("failed to load flatpak backend: {}", err);
            }
        }
    }

    #[cfg(feature = "packagekit")]
    {
        match packagekit::Packagekit::new(appstream_cache, locale) {
            Ok(backend) => {
                backends.insert("packagekit", Arc::new(backend));
            }
            Err(err) => {
                log::error!("failed to load packagekit backend: {}", err);
            }
        }
    }

    backends
}
