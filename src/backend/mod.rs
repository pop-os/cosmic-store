use cosmic::widget;
use rayon::prelude::*;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    sync::Arc,
    time::Instant,
};

use crate::{AppInfo, AppstreamCache, OperationKind};

#[cfg(feature = "flatpak")]
mod flatpak;

#[cfg(feature = "packagekit")]
mod packagekit;

#[derive(Clone, Debug)]
pub struct Package {
    pub id: String,
    pub icon: widget::icon::Handle,
    pub info: Arc<AppInfo>,
    pub version: String,
    pub extra: HashMap<String, String>,
}

pub trait Backend: fmt::Debug + Send + Sync {
    fn load_caches(&mut self) -> Result<(), Box<dyn Error>>;
    fn info_caches(&self) -> &[AppstreamCache];
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn operation(
        &self,
        kind: OperationKind,
        package_id: &str,
        info: &AppInfo,
        f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>>;
}

// BTreeMap for stable sort order
pub type Backends = BTreeMap<&'static str, Arc<dyn Backend>>;

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
        match Arc::get_mut(backend).unwrap().load_caches() {
            Ok(()) => {
                let duration = start.elapsed();
                log::info!("loaded {} backend caches in {:?}", backend_name, duration);
            }
            Err(err) => {
                log::error!("failed to load {} backend caches: {}", backend_name, err);
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
