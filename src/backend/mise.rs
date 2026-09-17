// Copyright 2026 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use cosmic::widget;
use serde::Deserialize;
use std::{
    collections::HashMap,
    error::Error,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::Arc,
};

use super::{Backend, Package};
use crate::{
    AppId, AppInfo, AppstreamCache, GStreamerCodec, Operation, OperationKind, app_info::AppRelease,
};

const SOURCE_ID: &str = "mise";
const SOURCE_NAME: &str = "Mise";
const GENERIC_ICON: &str = "package-x-generic";

/// Source of a tool request, as reported by `mise ls --json` and `mise outdated --json`
#[derive(Clone, Debug, Deserialize)]
struct ToolSource {
    #[serde(default)]
    path: Option<String>,
}

#[derive(Clone, Debug, Deserialize)]
struct ToolVersion {
    version: String,
    #[serde(default)]
    requested_version: Option<String>,
    #[serde(default)]
    source: Option<ToolSource>,
    #[serde(default)]
    installed: bool,
    #[serde(default)]
    active: bool,
}

#[derive(Debug, Deserialize)]
struct OutdatedTool {
    requested: String,
    #[serde(default)]
    current: Option<String>,
    latest: String,
    #[serde(default)]
    source: Option<ToolSource>,
}

#[derive(Clone, Debug, Deserialize)]
struct RegistryEntry {
    short: String,
    #[serde(default)]
    description: Option<String>,
    #[serde(default)]
    bins: Option<Vec<String>>,
}

/// Backend for developer tools installed with mise, scoped to the global mise configuration.
#[derive(Debug)]
pub struct Mise {
    binary: PathBuf,
    locale: String,
    /// Registry entries keyed by tool short name
    registry: HashMap<String, RegistryEntry>,
    appstream_caches: Vec<AppstreamCache>,
}

impl Mise {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        Ok(Self {
            binary: find_binary()?,
            locale: locale.to_string(),
            registry: HashMap::new(),
            appstream_caches: Vec::new(),
        })
    }

    fn run(&self, args: &[&str]) -> Result<String, Box<dyn Error>> {
        self.run_in(None, args)
    }

    fn run_in(&self, cwd: Option<&Path>, args: &[&str]) -> Result<String, Box<dyn Error>> {
        let mut command = Command::new(&self.binary);
        command
            .args(args)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped());
        if let Some(cwd) = cwd {
            command.current_dir(cwd);
        }
        let output = command.output()?;
        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!(
                "mise {} failed ({}): {}",
                args.join(" "),
                output.status,
                stderr.trim()
            )
            .into());
        }
        Ok(String::from_utf8_lossy(&output.stdout).to_string())
    }

    /// Installed versions requested by the global configuration, keyed by tool name
    fn global_installed(&self) -> Result<HashMap<String, ToolVersion>, Box<dyn Error>> {
        let stdout = self.run(&["ls", "-g", "-i", "--json"])?;
        if stdout.trim().is_empty() {
            return Ok(HashMap::new());
        }
        let tools: HashMap<String, Vec<ToolVersion>> = serde_json::from_str(&stdout)?;
        Ok(tools
            .into_iter()
            .filter_map(|(name, versions)| {
                let active = versions
                    .iter()
                    .rev()
                    .find(|version| version.installed && version.active)
                    .or_else(|| versions.iter().rev().find(|version| version.installed))?;
                Some((name, active.clone()))
            })
            .collect())
    }

    fn registry_entries(&self) -> Result<HashMap<String, RegistryEntry>, Box<dyn Error>> {
        let stdout = self.run(&["registry", "--json", "--hide-aliased"])?;
        if stdout.trim().is_empty() {
            return Ok(HashMap::new());
        }
        let entries: Vec<RegistryEntry> = serde_json::from_str(&stdout)?;
        Ok(entries
            .into_iter()
            .map(|entry| (entry.short.clone(), entry))
            .collect())
    }

    fn info(&self, name: &str, release: Option<AppRelease>) -> AppInfo {
        let entry = self.registry.get(name);
        let summary = entry
            .and_then(|entry| entry.description.clone())
            .unwrap_or_default();
        let mut description = if summary.is_empty() {
            format!("{name}, managed with mise.")
        } else {
            format!("{summary}\n\nManaged by mise.")
        };
        if let Some(bins) = entry.and_then(|entry| entry.bins.as_ref())
            && !bins.is_empty()
        {
            description.push_str("\n\nProvides: ");
            description.push_str(&bins.join(", "));
        }
        AppInfo {
            source_id: SOURCE_ID.to_string(),
            source_name: SOURCE_NAME.to_string(),
            name: name.to_string(),
            summary,
            description,
            releases: release.into_iter().collect(),
            search_only: true,
            ..Default::default()
        }
    }

    fn package(&self, name: &str, version: &str, extra: HashMap<String, String>) -> Package {
        Package {
            id: AppId::new(name),
            icon: widget::icon::from_name(GENERIC_ICON).size(128).handle(),
            info: Arc::new(self.info(name, None)),
            version: version.to_string(),
            extra,
        }
    }
}

impl Backend for Mise {
    fn load_caches(&mut self, _refresh: bool) -> Result<(), Box<dyn Error>> {
        let installed = self.global_installed()?;
        self.registry = self.registry_entries().unwrap_or_else(|err| {
            log::warn!("mise: failed to load registry: {err}");
            HashMap::new()
        });
        let mut infos = HashMap::new();
        for name in self.registry.keys() {
            infos.insert(AppId::new(name), Arc::new(self.info(name, None)));
        }
        for name in installed.keys() {
            infos
                .entry(AppId::new(name))
                .or_insert_with(|| Arc::new(self.info(name, None)));
        }
        log::info!(
            "mise: {} installed tools, {} registry tools",
            installed.len(),
            self.registry.len()
        );
        self.appstream_caches = vec![AppstreamCache {
            source_id: SOURCE_ID.to_string(),
            source_name: SOURCE_NAME.to_string(),
            locale: self.locale.clone(),
            infos,
            ..Default::default()
        }];
        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let mut packages = Vec::new();
        for (name, tool) in self.global_installed()? {
            let mut extra = HashMap::new();
            if let Some(requested) = &tool.requested_version {
                extra.insert("requested".to_string(), requested.clone());
            }
            if let Some(path) = tool.source.as_ref().and_then(|source| source.path.as_ref()) {
                extra.insert("config".to_string(), path.clone());
            }
            packages.push(self.package(&name, &tool.version, extra));
        }
        Ok(packages)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        let installed = self.global_installed()?;
        if installed.is_empty() {
            return Ok(Vec::new());
        }
        // Run from the filesystem root so only the global configuration contributes
        let stdout = self.run_in(Some(Path::new("/")), &["outdated", "--json"])?;
        if stdout.trim().is_empty() {
            return Ok(Vec::new());
        }
        let outdated: HashMap<String, OutdatedTool> = serde_json::from_str(&stdout)?;
        let mut packages = Vec::new();
        for (name, tool) in outdated {
            if !installed.contains_key(&name) {
                continue;
            }
            let current = tool.current.clone().unwrap_or_default();
            log::debug!(
                "mise: {name} update available ({current} -> {})",
                tool.latest
            );
            let mut extra = HashMap::new();
            extra.insert("current".to_string(), current.clone());
            extra.insert("requested".to_string(), tool.requested.clone());
            extra.insert("latest".to_string(), tool.latest.clone());
            if let Some(path) = tool.source.as_ref().and_then(|source| source.path.as_ref()) {
                extra.insert("config".to_string(), path.clone());
            }
            let release = AppRelease {
                timestamp: None,
                version: tool.latest.clone(),
                description: Some(if current.is_empty() {
                    format!("Latest version {}", tool.latest)
                } else {
                    format!("Update {current} → {}", tool.latest)
                }),
                url: None,
            };
            packages.push(Package {
                id: AppId::new(&name),
                icon: widget::icon::from_name(GENERIC_ICON).size(128).handle(),
                info: Arc::new(self.info(&name, Some(release))),
                version: tool.latest,
                extra,
            });
        }
        log::info!("mise: found {} updates", packages.len());
        Ok(packages)
    }

    fn file_packages(&self, _path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("mise backend does not support file-based package parsing".into())
    }

    fn gstreamer_packages(
        &self,
        _gstreamer_codec: &GStreamerCodec,
    ) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("mise backend does not support GStreamer codec queries".into())
    }

    #[allow(clippy::cast_precision_loss)]
    fn operation(
        &self,
        op: &Operation,
        mut f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        let names: Vec<&str> = op.package_ids.iter().map(AppId::raw).collect();
        if names.is_empty() {
            return Err("mise operation has no packages".into());
        }
        let total = names.len() as f32;
        match &op.kind {
            OperationKind::Install => {
                for (i, &name) in names.iter().enumerate() {
                    f(100.0 * i as f32 / total);
                    let arg = format!("{name}@latest");
                    log::info!("mise: installing {arg}");
                    self.run(&["-y", "use", "-g", arg.as_str()])?;
                }
                f(100.0);
                Ok(())
            }
            OperationKind::Uninstall { purge_data } => {
                for (i, &name) in names.iter().enumerate() {
                    f(100.0 * i as f32 / total);
                    let mut args = vec!["-y", "unuse", "-g"];
                    if !purge_data {
                        args.push("--no-prune");
                    }
                    args.push(name);
                    log::info!("mise: uninstalling {name}");
                    self.run(&args)?;
                }
                f(100.0);
                Ok(())
            }
            OperationKind::Update => {
                f(0.0);
                let mut args = vec!["-y", "upgrade"];
                args.extend(names.iter().copied());
                log::info!("mise: upgrading {names:?}");
                self.run(&args)?;
                f(100.0);
                Ok(())
            }
            OperationKind::RepositoryAdd { .. } | OperationKind::RepositoryRemove { .. } => {
                Err("mise backend does not support repository management".into())
            }
        }
    }
}

fn find_binary() -> Result<PathBuf, Box<dyn Error>> {
    if let Some(path) = std::env::var_os("MISE_BIN") {
        let path = PathBuf::from(path);
        if is_executable(&path) {
            return Ok(path);
        }
        return Err(format!("MISE_BIN is not an executable file: {}", path.display()).into());
    }

    let mut candidates: Vec<PathBuf> = Vec::new();
    if let Some(paths) = std::env::var_os("PATH") {
        candidates.extend(std::env::split_paths(&paths).map(|dir| dir.join("mise")));
    }
    if let Some(home) = std::env::var_os("HOME") {
        let home = PathBuf::from(home);
        candidates.push(home.join(".local/bin/mise"));
        candidates.push(home.join(".cargo/bin/mise"));
        candidates.push(home.join(".local/share/mise/shims/mise"));
    }
    candidates.push(PathBuf::from("/usr/local/bin/mise"));
    candidates.push(PathBuf::from("/usr/bin/mise"));

    candidates
        .into_iter()
        .find(|path| is_executable(path))
        .ok_or_else(|| "mise command not found; install mise to manage developer tools".into())
}

fn is_executable(path: &Path) -> bool {
    use std::os::unix::fs::PermissionsExt;
    std::fs::metadata(path)
        .is_ok_and(|metadata| metadata.is_file() && metadata.permissions().mode() & 0o111 != 0)
}
