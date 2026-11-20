use cosmic_theme_bundle_config::Store;
use sha2::Digest;
use std::{
    collections::{HashMap, HashSet},
    error::Error,
    fs,
    io::Write,
    path::PathBuf,
    time::Instant,
};

use super::{Backend, Package};
use crate::{
    AppIcon, AppstreamCache, Operation, app_id::AppId, fl, operation::OperationKind,
    preview_cache::ICONS,
};
use futures::StreamExt;

#[derive(Debug)]
pub struct Constellations {
    url: String,
    appstream_caches: Vec<AppstreamCache>,
    store: Store,
}

impl Constellations {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        let mut this = Self {
            url: "https://apt.pop-os.org/cosmic-constellations".to_string(),
            appstream_caches: Vec::new(),
            store: Store::new()?,
        };
        this.update_catalog(false)?;
        this.appstream_caches.push(AppstreamCache::new(
            "constellations".to_string(),
            fl!("constellations"),
            vec![Self::catalog_path()?],
            vec![],
            locale,
        ));
        this.download_icons();
        Ok(this)
    }

    fn catalog_path() -> Result<PathBuf, Box<dyn Error>> {
        let cache_dir = dirs::cache_dir()
            .map(|x| x.join("cosmic-constellations"))
            .ok_or("failed to get cosmic-constellations cache_dir")?;
        Ok(cache_dir.join("catalog.xml"))
    }

    /// Fetch the remote icons the catalog lists that are not cached yet.
    fn download_icons(&self) {
        let missing: Vec<(String, String)> = self
            .appstream_caches
            .iter()
            .flat_map(|cache| cache.infos.values())
            .flat_map(|info| {
                info.icons.iter().filter_map(|icon| match icon {
                    AppIcon::Remote(url, ..) => Some((url.clone(), info.version().to_owned())),
                    _ => None,
                })
            })
            .filter(|(url, version)| !ICONS.path(url, version).is_some_and(|p| p.is_file()))
            .collect();
        if missing.is_empty() {
            return;
        }
        let Ok(runtime) = tokio::runtime::Handle::try_current() else {
            log::warn!("no async runtime to download {} icons on", missing.len());
            return;
        };
        runtime.spawn(async move {
            futures::stream::iter(missing)
                .for_each_concurrent(8, |(url, version)| async move {
                    let bytes = match reqwest::get(&url).await.and_then(|r| r.error_for_status()) {
                        Ok(response) => response.bytes().await,
                        Err(err) => Err(err),
                    };
                    match bytes {
                        Ok(bytes) => {
                            if let Err(err) = ICONS.save_to_cache(&url, &version, &bytes) {
                                log::warn!("failed to cache icon {}: {}", url, err);
                            }
                        }
                        Err(err) => log::warn!("failed to download icon {}: {}", url, err),
                    }
                })
                .await;
            ICONS.enforce_size_limit();
        });
    }

    fn update_catalog(&self, refresh: bool) -> Result<bool, Box<dyn Error>> {
        let catalog_path = Self::catalog_path()?;
        if !refresh && catalog_path.exists() {
            return Ok(false);
        }
        if let Some(parent) = catalog_path.parent() {
            fs::create_dir_all(parent)?;
        }

        let bytes = futures::executor::block_on(async {
            reqwest::get(format!("{}/catalog.xml", self.url))
                .await?
                .error_for_status()?
                .bytes()
                .await
        })?;

        atomicwrites::AtomicFile::new(
            &catalog_path,
            atomicwrites::OverwriteBehavior::AllowOverwrite,
        )
        .write(|file| file.write_all(&bytes))?;

        Ok(true)
    }
}

impl Backend for Constellations {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>> {
        self.update_catalog(refresh)?;
        for appstream_cache in self.appstream_caches.iter_mut() {
            appstream_cache.reload();
            eprintln!("{:?}", appstream_cache.infos);
        }
        self.download_icons();
        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let appstream_cache = &self.appstream_caches[0];

        let mut installed = Vec::new();
        for bundle in self.store.bundles()? {
            let Some(package) = &bundle.package else {
                continue;
            };
            let id = AppId::new(&package.package_id);
            let Some(info) = appstream_cache.infos.get(&id) else {
                continue;
            };
            installed.push(Package {
                id: id.clone(),
                icon: appstream_cache.icon(info),
                info: info.clone(),
                version: package.version.clone(),
                extra: HashMap::new(),
            });
        }
        Ok(installed)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        //TODO: constellation updates
        Ok(Vec::new())
    }

    fn file_packages(&self, _path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("Constellations::file_packages not implemented".into())
    }

    fn operation(
        &self,
        op: &Operation,
        mut callback: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        match op.kind {
            OperationKind::Install => {
                log::info!("installing packages {:?}", op.package_ids);

                let mut artifacts = Vec::with_capacity(op.package_ids.len());
                for (package_id, info) in op.package_ids.iter().zip(op.infos.iter()) {
                    let Some(release) = info.releases.first() else {
                        return Err(format!("{} does not have any releases", info.name).into());
                    };
                    let Some(artifact) = release.artifacts.first() else {
                        return Err(format!(
                            "{} release {} does not have any artifacts",
                            info.name, release.version
                        )
                        .into());
                    };
                    let Some(sha256) = artifact.sha256.clone() else {
                        return Err(format!(
                            "{} release {} artifact does not have sha256 hash",
                            info.name, release.version
                        )
                        .into());
                    };
                    let Some(size) = artifact.size else {
                        return Err(format!(
                            "{} release {} artifact does not have download size",
                            info.name, release.version
                        )
                        .into());
                    };
                    artifacts.push((package_id.clone(), artifact.url.clone(), sha256, size))
                }

                let mut total_progress = 0.0;
                let progress_increment = 100.0 / (artifacts.len() as f32);
                for (package_id, url, sha256, size) in artifacts.iter() {
                    //TODO: validate url is in self.url?
                    //TODO: size limit?
                    let mut response =
                        futures::executor::block_on(async { reqwest::get(url).await })?
                            .error_for_status()?;
                    if response.content_length() != Some(*size) {
                        return Err(format!(
                            "{} content length {:?} does not match expected {}",
                            url,
                            response.content_length(),
                            size
                        )
                        .into());
                    }

                    //TODO: stream to file not memory?
                    let mut file = tempfile::NamedTempFile::with_prefix("cosmic-constellations")?;
                    let mut written = 0;
                    let mut hasher = sha2::Sha256::new();
                    let mut progress_time = Instant::now();
                    while let Some(chunk) =
                        futures::executor::block_on(async { response.chunk().await })?
                    {
                        hasher.update(&chunk);
                        file.write_all(&chunk)?;
                        written += u64::try_from(chunk.len())?;
                        if written > *size {
                            // Too large, break and trigger size error below
                            break;
                        }

                        // Throttle progress
                        if progress_time.elapsed().as_secs_f32() > 0.1 {
                            callback(
                                total_progress
                                    + progress_increment * (written as f32) / (*size as f32),
                            );
                            progress_time = Instant::now();
                        }
                    }

                    // Verify size
                    if written != *size {
                        return Err(format!(
                            "{} size {:?} does not match expected {}",
                            url, written, size
                        )
                        .into());
                    }

                    // Verify hash
                    let data_sha256 = format!("{:x}", hasher.finalize());
                    if &data_sha256 != sha256 {
                        return Err(format!(
                            "{} sha256 {} does not match expected {}",
                            url, data_sha256, sha256
                        )
                        .into());
                    }

                    total_progress += progress_increment;
                    callback(total_progress);

                    //TODO: progress should be reported during import
                    self.store.import_archive(file.path(), None)?;
                }

                Ok(())
            }
            OperationKind::Uninstall { purge_data } => {
                log::info!(
                    "uninstalling packages {:?} (purge_data: {})",
                    op.package_ids,
                    purge_data
                );

                let mut slugs = HashSet::with_capacity(op.package_ids.len());
                for bundle in self.store.bundles()? {
                    let Some(package) = bundle.package else {
                        continue;
                    };
                    for package_id in op.package_ids.iter() {
                        if &AppId::new(&package.package_id) == package_id {
                            slugs.insert(bundle.slug.clone());
                            break;
                        }
                    }
                }

                let mut total_progress = 0.0;
                let progress_increment = 100.0 / (slugs.len() as f32);
                for slug in slugs {
                    self.store.delete(&slug)?;
                    total_progress += progress_increment;
                    callback(total_progress);
                }

                Ok(())
            }
            _ => Err(format!("{:?} not supported for constellations", op.kind).into()),
        }
    }
}
