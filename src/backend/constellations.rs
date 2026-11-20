use sha2::Digest;
use std::{error::Error, fs, io::Write, path::PathBuf, time::Instant};

use super::{Backend, Package};
use crate::{AppstreamCache, Operation, operation::OperationKind};

#[derive(Debug)]
pub struct Constellations {
    url: String,
    appstream_caches: Vec<AppstreamCache>,
}

impl Constellations {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        let mut this = Self {
            url: "https://apt.pop-os.org/cosmic-constellations".to_string(),
            appstream_caches: Vec::new(),
        };
        this.update_catalog(false)?;
        this.appstream_caches.push(AppstreamCache::new(
            "constellations".to_string(),
            "Constellations".to_string(),
            vec![Self::catalog_path()?],
            vec![],
            locale,
        ));
        Ok(this)
    }

    fn catalog_path() -> Result<PathBuf, Box<dyn Error>> {
        let cache_dir = dirs::cache_dir()
            .map(|x| x.join("cosmic-constellations"))
            .ok_or("failed to get cosmic-constellations cache_dir")?;
        Ok(cache_dir.join("catalog.xml"))
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
        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        //TODO
        Ok(Vec::new())
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        //TODO
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
                let mut artifacts = Vec::with_capacity(op.infos.len());
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
                    let mut data = Vec::new();
                    let mut hasher = sha2::Sha256::new();
                    let mut progress_time = Instant::now();
                    while let Some(chunk) =
                        futures::executor::block_on(async { response.chunk().await })?
                    {
                        hasher.update(&chunk);
                        data.extend_from_slice(&chunk);
                        if u64::try_from(data.len())? > *size {
                            // Too large, break and trigger size error below
                            break;
                        }

                        let progress = total_progress
                            + progress_increment * (data.len() as f32) / (*size as f32);
                        if progress_time.elapsed().as_secs_f32() > 0.1 {
                            // Throttle progress
                            callback(progress);
                            progress_time = Instant::now();
                        }
                    }
                    if u64::try_from(data.len())? != *size {
                        return Err(format!(
                            "{} size {:?} does not match expected {}",
                            url,
                            data.len(),
                            size
                        )
                        .into());
                    }

                    let data_sha256 = format!("{:x}", hasher.finalize());
                    if &data_sha256 != sha256 {
                        return Err(format!(
                            "{} sha256 {} does not match expected {}",
                            url, data_sha256, sha256
                        )
                        .into());
                    }

                    //TODO: actually install, not just download

                    total_progress += progress_increment;
                    callback(total_progress)
                }

                Ok(())
            }
            _ => Err(format!("{:?} not supported for constellations", op.kind).into()),
        }
    }
}
