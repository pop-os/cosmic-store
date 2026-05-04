use cosmic::widget;
use std::{
    collections::HashMap,
    error::Error,
    process::{Command, Stdio},
    sync::Arc,
};

use super::{Backend, Package};
use crate::{AppId, AppInfo, AppstreamCache, GStreamerCodec, Operation, OperationKind};

#[derive(Debug)]
pub struct RpmOstree {
    appstream_caches: Vec<AppstreamCache>,
}

impl RpmOstree {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        let source_id = "rpm-ostree";
        //TODO: translate?
        let source_name = "System";
        Ok(Self {
            appstream_caches: vec![AppstreamCache::system(
                source_id.to_string(),
                source_name.to_string(),
                locale,
            )],
        })
    }

    fn parse_staged_commit(&self, output: &str) -> Result<Option<String>, Box<dyn Error>> {
        let parts: Vec<&str> = output.split("\u{25cf} ").collect();

        // If there's more than one part, something is staged before the ●
        if parts.len() >= 2 {
            let staged_section = parts[0];
            for line in staged_section.lines() {
                let line = line.trim();
                if line.starts_with("Commit:") || line.starts_with("BaseCommit:") {
                    if let Some(commit) = line.split(':').nth(1).map(|s| s.trim().to_string()) {
                        return Ok(Some(commit));
                    }
                }
            }
        }

        Ok(None)
    }

    fn parse_upgrade_check(
        &self,
        output: &str,
        staged_commit: Option<&str>,
    ) -> Result<Vec<Package>, Box<dyn Error>> {
        if !output.contains("AvailableUpdate:") {
            return Ok(Vec::new()); // No updates available
        }

        let commit = output
            .lines()
            .find(|line| line.trim().starts_with("Commit:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|s| s.trim().to_string());

        let version = output
            .lines()
            .find(|line| line.trim().starts_with("Version:"))
            .and_then(|line| line.split(':').nth(1))
            .map(|s| s.trim())
            .and_then(|line| line.split(' ').nth(0))
            .map(|s| s.trim().to_string());

        let (Some(commit), Some(version)) = (commit, version) else {
            return Ok(Vec::new());
        };

        // If the available update matches the staged version by commit, we have already
        // queued the update.
        if staged_commit == Some(commit.as_str()) {
            log::debug!(
                "Update commit {} is already queued, not showing in UI",
                commit
            );
            return Ok(Vec::new());
        }

        let mut extra = HashMap::new();
        extra.insert("version".to_string(), version.clone());
        extra.insert("commit".to_string(), commit.clone());

        Ok(vec![Package {
            id: AppId::system(),
            icon: widget::icon::from_name("package-x-generic")
                .size(128)
                .handle(),
            info: Arc::new(AppInfo {
                source_id: "rpm-ostree".to_string(),
                source_name: "rpm-ostree".to_string(),
                name: "System Upgrade".to_string(),
                summary: format!("Update to version {}", version),
                description: "An OS update is available via rpm-ostree.".to_string(),
                pkgnames: vec!["system".to_string()],
                ..Default::default()
            }),
            version,
            extra,
        }])
    }

    fn run_command(&self, args: &[&str]) -> Result<String, Box<dyn Error>> {
        // Check if rpm-ostree is available
        if !std::path::Path::new("/usr/bin/rpm-ostree").exists() {
            return Err("rpm-ostree command not found. Please install rpm-ostree package.".into());
        }

        let output = Command::new("rpm-ostree")
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "rpm-ostree command failed with status {}: {}",
                output.status, stderr
            )
            .into());
        }

        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }
}

impl Backend for RpmOstree {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>> {
        if refresh {
            // Refresh appstream cache
            log::info!("Refreshing rpm-ostree appstream cache");
            for cache in self.appstream_caches.iter_mut() {
                cache.reload();
            }
        }

        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        // while rpm-ostree supports package layering,
        // we are focusing on system updates only
        Ok(Vec::new())
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let status_output = self.run_command(&["status"])?;
        let staged_version = self.parse_staged_commit(&status_output)?;

        let output = self.run_command(&["upgrade", "--check"])?;
        self.parse_upgrade_check(&output, staged_version.as_deref())
    }

    fn file_packages(&self, _path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("rpm-ostree backend does not support file-based package parsing".into())
    }

    fn gstreamer_packages(
        &self,
        _gstreamer_codec: &GStreamerCodec,
    ) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("rpm-ostree backend does not support GStreamer codec queries".into())
    }

    fn operation(
        &self,
        op: &Operation,
        mut f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        match &op.kind {
            OperationKind::Update => {
                f(0.0);
                log::info!("Applying rpm-ostree system update");
                let _ = self.run_command(&["upgrade"])?;
                f(100.0);
                log::info!("rpm-ostree upgrade completed");
                Ok(())
            }
            // rpm-ostree supports package layering, but we only care about updates
            OperationKind::Install
            | OperationKind::Uninstall { .. }
            | OperationKind::RepositoryAdd { .. }
            | OperationKind::RepositoryRemove { .. } => {
                Err("rpm-ostree backend does not support per-package operations or repository management".into())
            }
        }
    }
}
