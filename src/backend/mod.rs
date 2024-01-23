use cosmic::widget;
use std::{collections::HashMap, error::Error, fmt, sync::Arc, time::Instant};

use crate::{AppInfo, AppstreamCache};

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
    //TODO: remove
    fn info(&self, package: &Package) -> Result<Arc<AppInfo>, Box<dyn Error>> {
        let info_cache = self.info_cache();
        match info_cache.infos.get(&package.id) {
            Some(info) => Ok(info.clone()),
            None => Err(format!("failed to find info for {}", package.id).into()),
        }
    }
    fn info_cache(&self) -> &Arc<AppstreamCache>;
}

pub type Backends = HashMap<&'static str, Arc<dyn Backend>>;

pub fn backends(locale: &str) -> Backends {
    let mut backends = Backends::new();

    #[cfg(feature = "flatpak")]
    {
        let start = Instant::now();
        match flatpak::Flatpak::new(locale) {
            Ok(backend) => {
                backends.insert("flatpak", Arc::new(backend));
                let duration = start.elapsed();
                log::info!("loaded flatpak backend in {:?}", duration);
            }
            Err(err) => {
                log::error!("failed to load flatpak backend: {}", err);
            }
        }
    }

    #[cfg(feature = "packagekit")]
    {
        let start = Instant::now();
        match packagekit::Packagekit::new(locale) {
            Ok(backend) => {
                backends.insert("packagekit", Arc::new(backend));
                let duration = start.elapsed();
                log::info!("loaded packagekit backend in {:?}", duration);
            }
            Err(err) => {
                log::error!("failed to load packagekit backend: {}", err);
            }
        }
    }

    //TODO: Workaround for xml-rs memory leak when loading appstream data
    {
        let start = Instant::now();
        unsafe {
            libc::malloc_trim(0);
        }
        let duration = start.elapsed();
        log::info!("trimmed allocations in {:?}", duration);
    }

    backends
}
