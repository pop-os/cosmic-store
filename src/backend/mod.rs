use cosmic::widget;
use rayon::prelude::*;
use std::{
    collections::{BTreeMap, HashMap},
    error::Error,
    fmt,
    sync::Arc,
    time::Instant,
};

use crate::{AppId, AppInfo, AppstreamCache, GStreamerCodec, Operation};

/// Enum representing the available backend types
#[derive(Clone, Copy, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub enum BackendName {
    FlatpakUser,
    FlatpakSystem,
    Homebrew,
    Packagekit,
    Pkgar,
}

impl BackendName {
    /// Returns the string representation of the backend name
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendName::FlatpakUser => "flatpak-user",
            BackendName::FlatpakSystem => "flatpak-system",
            BackendName::Homebrew => "homebrew",
            BackendName::Packagekit => "packagekit",
            BackendName::Pkgar => "pkgar",
        }
    }

    /// Check if this is a flatpak backend
    pub fn is_flatpak(&self) -> bool {
        matches!(self, BackendName::FlatpakUser | BackendName::FlatpakSystem)
    }
}

impl fmt::Display for BackendName {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.as_str())
    }
}

impl std::str::FromStr for BackendName {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "flatpak-user" => Ok(BackendName::FlatpakUser),
            "flatpak-system" => Ok(BackendName::FlatpakSystem),
            "homebrew" => Ok(BackendName::Homebrew),
            "packagekit" => Ok(BackendName::Packagekit),
            "pkgar" => Ok(BackendName::Pkgar),
            _ => Err(format!("unknown backend name: {}", s)),
        }
    }
}

#[cfg(feature = "flatpak")]
mod flatpak;

#[cfg(feature = "packagekit")]
mod packagekit;

#[cfg(feature = "pkgar")]
mod pkgar;

#[cfg(feature = "homebrew")]
mod homebrew;

#[derive(Clone, Debug)]
pub struct Package {
    pub id: AppId,
    pub icon: widget::icon::Handle,
    pub info: Arc<AppInfo>,
    pub version: String,
    pub extra: HashMap<String, String>,
}

pub trait Backend: fmt::Debug + Send + Sync {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>>;
    fn info_caches(&self) -> &[AppstreamCache];
    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>>;
    fn file_packages(&self, path: &str) -> Result<Vec<Package>, Box<dyn Error>>;
    fn gstreamer_packages(
        &self,
        _gstreamer_codec: &GStreamerCodec,
    ) -> Result<Vec<Package>, Box<dyn Error>> {
        Ok(Vec::new())
    }
    fn operation(
        &self,
        op: &Operation,
        f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>>;
}

// BTreeMap for stable sort order
pub type Backends = BTreeMap<BackendName, Arc<dyn Backend>>;

pub fn backends(locale: &str, refresh: bool) -> Backends {
    let mut backends = Backends::new();

    #[cfg(feature = "flatpak")]
    {
        for (backend_name, user) in [
            (BackendName::FlatpakUser, true),
            (BackendName::FlatpakSystem, false),
        ] {
            let start = Instant::now();
            match flatpak::Flatpak::new(user, locale) {
                Ok(backend) => {
                    backends.insert(backend_name, Arc::new(backend));
                    let duration = start.elapsed();
                    log::info!("initialized {} backend in {:?}", backend_name, duration);
                }
                Err(err) => {
                    log::error!("failed to load {} backend: {}", backend_name, err);
                }
            }
        }
    }

    #[cfg(feature = "packagekit")]
    {
        let start = Instant::now();
        match packagekit::Packagekit::new(locale) {
            Ok(backend) => {
                backends.insert(BackendName::Packagekit, Arc::new(backend));
                let duration = start.elapsed();
                log::info!(
                    "initialized {} backend in {:?}",
                    BackendName::Packagekit,
                    duration
                );
            }
            Err(err) => {
                log::error!(
                    "failed to load {} backend: {}",
                    BackendName::Packagekit,
                    err
                );
            }
        }
    }

    #[cfg(feature = "pkgar")]
    {
        let start = Instant::now();
        match pkgar::Pkgar::new(locale) {
            Ok(backend) => {
                backends.insert(BackendName::Pkgar, Arc::new(backend));
                let duration = start.elapsed();
                log::info!(
                    "initialized {} backend in {:?}",
                    BackendName::Pkgar,
                    duration
                );
            }
            Err(err) => {
                log::error!("failed to load {} backend: {}", BackendName::Pkgar, err);
            }
        }
    }

    // Note: Homebrew is initialized separately via init_homebrew() to avoid
    // delaying the explore page load (homebrew has no appstream data)

    backends.par_iter_mut().for_each(|(backend_name, backend)| {
        let start = Instant::now();
        match Arc::get_mut(backend).unwrap().load_caches(refresh) {
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
    #[cfg(all(target_os = "linux", target_env = "gnu"))]
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

/// Initialize homebrew backend separately (deferred to avoid delaying explore page)
#[cfg(feature = "homebrew")]
pub fn init_homebrew(locale: &str) -> Option<Arc<dyn Backend>> {
    let start = Instant::now();
    match homebrew::Homebrew::new(locale) {
        Ok(backend) => {
            let duration = start.elapsed();
            log::info!("initialized homebrew backend in {:?}", duration);
            Some(Arc::new(backend))
        }
        Err(err) => {
            log::debug!("homebrew backend not available: {}", err);
            None
        }
    }
}

#[cfg(not(feature = "homebrew"))]
pub fn init_homebrew(_locale: &str) -> Option<Arc<dyn Backend>> {
    None
}
