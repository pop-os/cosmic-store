use cosmic::widget;
use futures::StreamExt;
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
    Packagekit,
    Pkgar,
}

impl BackendName {
    /// Returns the string representation of the backend name
    pub fn as_str(&self) -> &'static str {
        match self {
            BackendName::FlatpakUser => "flatpak-user",
            BackendName::FlatpakSystem => "flatpak-system",
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

/// Load store backends using rayon parallelism and concurrency.
pub fn backends<'a>(
    locale: &'a str,
    refresh: bool,
) -> impl futures::Stream<Item = (BackendName, Arc<dyn Backend>)> + Send + Unpin + 'static {
    let backends = futures::stream::FuturesUnordered::new();

    #[cfg(feature = "flatpak")]
    {
        for (backend_name, user) in [
            (BackendName::FlatpakUser, true),
            (BackendName::FlatpakSystem, false),
        ] {
            let (tx, rx) = tokio::sync::oneshot::channel();
            let locale = locale.to_owned();

            tokio::task::spawn_blocking(move || {
                let start = Instant::now();
                log::info!("adding flatpak repository");
                _ = tx.send(match flatpak::Flatpak::new(user, &locale) {
                    Ok(backend) => {
                        let duration = start.elapsed();
                        log::warn!("initialized {} backend in {:?}", backend_name, duration);
                        let backend: Arc<dyn Backend> = Arc::new(backend);
                        Some((backend_name, backend))
                    }
                    Err(err) => {
                        log::warn!("failed to load {} backend: {}", backend_name, err);
                        None
                    }
                });
            });

            backends.push(rx)
        }
    }

    #[cfg(feature = "packagekit")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let locale = locale.to_owned();

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            log::info!("adding packagekit backend");
            _ = tx.send(match packagekit::Packagekit::new(&locale) {
                Ok(backend) => {
                    let backend: Arc<dyn Backend> = Arc::new(backend);
                    let duration = start.elapsed();
                    log::warn!(
                        "initialized {} backend in {:?}",
                        BackendName::Packagekit,
                        duration
                    );
                    Some((BackendName::Packagekit, backend))
                }
                Err(err) => {
                    log::error!(
                        "failed to load {} backend: {}",
                        BackendName::Packagekit,
                        err
                    );
                    None
                }
            });
        });

        backends.push(rx)
    }

    #[cfg(feature = "pkgar")]
    {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let locale = locale.to_owned();

        tokio::task::spawn_blocking(move || {
            let start = Instant::now();
            log::info!("adding pkgar backend");
            _ = tx.send(match pkgar::Pkgar::new(&locale) {
                Ok(backend) => {
                    let backend: Arc<dyn Backend> = Arc::new(backend);
                    let duration = start.elapsed();
                    log::info!(
                        "initialized {} backend in {:?}",
                        BackendName::Pkgar,
                        duration
                    );
                    Some((BackendName::Pkgar, backend))
                }
                Err(err) => {
                    log::error!("failed to load {} backend: {}", BackendName::Pkgar, err);
                    None
                }
            })
        });

        backends.push(rx)
    }

    backends
        // Create a stream of futures that wait for caches to be loaded for each backend received
        .map(move |value| async move {
            let (backend_name, mut backend) = value.ok()??;

            let start = Instant::now();

            let (tx, rx) = tokio::sync::oneshot::channel();

            tokio::task::spawn_blocking(move || {
                match Arc::get_mut(&mut backend).unwrap().load_caches(refresh) {
                    Ok(()) => {
                        let duration = start.elapsed();
                        log::info!("loaded {} backend caches in {:?}", backend_name, duration);
                    }
                    Err(err) => {
                        log::error!("failed to load {} backend caches: {}", backend_name, err);
                    }
                }

                _ = tx.send(backend);
            });

            Some((backend_name, rx.await.unwrap()))
        })
        // Concurrently load caches for up to 4 backends at one time
        .buffer_unordered(4)
        // After all backends have been loaded, trim malloc
        .chain(futures::stream::once(async {
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

            None
        }))
        // Filter None outcomes
        .filter_map(|result| async move { result })
        // Pin it for `StreamExt::next()`.
        .boxed()
}
