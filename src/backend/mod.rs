use cosmic::widget;
use rayon::prelude::*;
use std::{collections::HashMap, error::Error, fmt, sync::Arc, time::Instant};

use crate::{AppInfo, AppstreamCache, OperationKind};

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
    pub origin_opt: Option<String>,
    pub version: String,
    pub extra: HashMap<String, String>,
}

pub trait Backend: fmt::Debug + Send + Sync {
    fn load_cache(&mut self) -> Result<(), Box<dyn Error>>;
    //TODO: remove
    fn info(&self, package: &Package) -> Result<Arc<AppInfo>, Box<dyn Error>> {
        let info_cache = self.info_cache();
        match info_cache.infos.get(&package.id) {
            Some(info) => Ok(info.clone()),
            None => Err(format!("failed to find info for {}", package.id).into()),
        }
    }
    fn info_cache(&self) -> &AppstreamCache;
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn operation(
        &self,
        kind: OperationKind,
        _package_id: &str,
        _f: Box<dyn FnMut(f32)>,
    ) -> Result<(), Box<dyn Error>> {
        Err(format!("{kind:?} not implemented for backend").into())
    }
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
                log::info!("initialized flatpak backend in {:?}", duration);
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
                log::info!("initialized packagekit backend in {:?}", duration);
            }
            Err(err) => {
                log::error!("failed to load packagekit backend: {}", err);
            }
        }
    }

    backends.par_iter_mut().for_each(|(backend_name, backend)| {
        let start = Instant::now();
        match Arc::get_mut(backend).unwrap().load_cache() {
            Ok(()) => {
                let duration = start.elapsed();
                log::info!("loaded {} backend cache in {:?}", backend_name, duration);
            }
            Err(err) => {
                log::error!("failed to load {} backend cache: {}", backend_name, err);
            }
        }
    });

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
