use cosmic::widget;
use serde::Deserialize;
use std::{
    collections::HashMap,
    error::Error,
    fmt::Write,
    io::{BufRead, BufReader},
    process::{Command, Stdio},
    sync::{Arc, Mutex},
};

use super::{Backend, Package};
use crate::{AppId, AppInfo, AppUrl, AppstreamCache, Operation, OperationKind};

// Environment variables to suppress Homebrew auto-update and hints
const BREW_ENV: [(&str, &str); 2] = [
    ("HOMEBREW_NO_AUTO_UPDATE", "1"),
    ("HOMEBREW_NO_ENV_HINTS", "1"),
];

/// JSON structure for `brew info --json=v2 --installed`
#[derive(Debug, Deserialize)]
struct BrewInfoOutput {
    formulae: Vec<BrewFormula>,
    #[serde(default)]
    casks: Vec<BrewCask>,
}

#[derive(Debug, Deserialize)]
struct BrewFormula {
    #[allow(dead_code)]
    name: String,
    full_name: String,
    #[allow(dead_code)]
    desc: Option<String>,
    #[allow(dead_code)]
    homepage: Option<String>,
    installed: Vec<BrewInstalled>,
}

#[derive(Debug, Deserialize)]
struct BrewInstalled {
    version: String,
}

#[derive(Debug, Deserialize)]
struct BrewCask {
    token: String,
    name: Vec<String>,
    desc: Option<String>,
    homepage: Option<String>,
    installed: Option<String>,
}

/// JSON structure for `brew outdated --json`
#[derive(Debug, Deserialize)]
struct BrewOutdatedOutput {
    #[serde(default)]
    formulae: Vec<BrewOutdatedFormula>,
    #[serde(default)]
    casks: Vec<BrewOutdatedCask>,
}

#[derive(Debug, Deserialize)]
struct BrewOutdatedFormula {
    name: String,
    installed_versions: Vec<String>,
    current_version: String,
    #[serde(default)]
    pinned: bool,
}

#[derive(Debug, Deserialize)]
struct BrewOutdatedCask {
    name: String,
    installed_versions: String,
    current_version: String,
}

/// Cached results from brew commands (info and outdated run in parallel)
struct BrewCache {
    info: BrewInfoOutput,
    outdated: BrewOutdatedOutput,
}

pub struct Homebrew {
    brew_path: String,
    appstream_caches: Vec<AppstreamCache>,
    /// Cache for brew command results - populated on first access
    cache: Mutex<Option<BrewCache>>,
}

impl std::fmt::Debug for Homebrew {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Homebrew")
            .field("brew_path", &self.brew_path)
            .field("appstream_caches", &self.appstream_caches)
            .finish()
    }
}

impl Homebrew {
    pub fn new(locale: &str) -> Result<Self, Box<dyn Error>> {
        let brew_path = Self::detect_brew_path().ok_or("Homebrew not found")?;

        log::info!("Found Homebrew at: {}", brew_path);

        // Verify brew is functional
        let output = Command::new(&brew_path)
            .arg("--version")
            .envs(BREW_ENV)
            .output()?;

        if !output.status.success() {
            return Err("Homebrew installation appears broken".into());
        }

        let version = String::from_utf8_lossy(&output.stdout);
        log::info!(
            "Homebrew version: {}",
            version.lines().next().unwrap_or("unknown")
        );

        Ok(Self {
            brew_path,
            appstream_caches: vec![AppstreamCache {
                source_id: "homebrew".to_string(),
                source_name: "Homebrew".to_string(),
                locale: locale.to_string(),
                ..Default::default()
            }],
            cache: Mutex::new(None),
        })
    }

    /// Ensure cache is populated by running both brew commands in parallel
    fn ensure_cache(&self) -> Result<(), Box<dyn Error>> {
        let mut cache = self.cache.lock().unwrap();
        if cache.is_some() {
            return Ok(());
        }

        // Run both brew commands in parallel (convert errors to strings for Send safety)
        let (info_result, outdated_result) = rayon::join(
            || {
                self.run_brew(&["info", "--json=v2", "--installed"])
                    .map_err(|e| e.to_string())
            },
            || {
                self.run_brew(&["outdated", "--json"])
                    .map_err(|e| e.to_string())
            },
        );

        let info_output = info_result.map_err(|e| -> Box<dyn Error> { e.into() })?;
        let outdated_output = outdated_result.map_err(|e| -> Box<dyn Error> { e.into() })?;

        let info: BrewInfoOutput = serde_json::from_slice(&info_output).map_err(|e| {
            log::error!("Failed to parse brew info JSON: {}", e);
            e
        })?;

        let outdated: BrewOutdatedOutput =
            serde_json::from_slice(&outdated_output).map_err(|e| {
                log::error!("Failed to parse brew outdated JSON: {}", e);
                e
            })?;

        *cache = Some(BrewCache { info, outdated });
        Ok(())
    }

    /// Clear the cache (called on refresh)
    fn clear_cache(&self) {
        let mut cache = self.cache.lock().unwrap();
        *cache = None;
    }

    /// Detect brew in PATH or common locations
    fn detect_brew_path() -> Option<String> {
        // Check if 'brew' is in PATH
        if let Ok(output) = Command::new("which").arg("brew").output() {
            if output.status.success() {
                let path = String::from_utf8_lossy(&output.stdout).trim().to_string();
                if !path.is_empty() {
                    return Some(path);
                }
            }
        }

        // Check common installation locations
        for path in [
            "/home/linuxbrew/.linuxbrew/bin/brew",
            "/opt/homebrew/bin/brew",
            "/usr/local/bin/brew",
        ] {
            if std::path::Path::new(path).exists() {
                return Some(path.to_string());
            }
        }

        None
    }

    /// Execute a brew command and return stdout
    fn run_brew(&self, args: &[&str]) -> Result<Vec<u8>, Box<dyn Error>> {
        log::debug!("running: brew {}", args.join(" "));

        let output = Command::new(&self.brew_path)
            .args(args)
            .envs(BREW_ENV)
            .output()?;

        if !output.status.success() {
            let stderr = String::from_utf8_lossy(&output.stderr);
            return Err(format!("brew {} failed: {}", args.join(" "), stderr).into());
        }

        Ok(output.stdout)
    }

    /// Execute a brew command with progress reporting
    fn run_brew_with_progress(
        &self,
        args: &[&str],
        mut on_progress: impl FnMut(f32),
    ) -> Result<(), Box<dyn Error>> {
        log::info!("running: brew {}", args.join(" "));

        let mut child = Command::new(&self.brew_path)
            .args(args)
            .envs(BREW_ENV)
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .spawn()?;

        on_progress(5.0);

        // Read stdout for progress estimation
        if let Some(stdout) = child.stdout.take() {
            let reader = BufReader::new(stdout);
            let mut lines = 0;

            for line in reader.lines().map_while(Result::ok) {
                log::debug!("brew: {}", line);
                lines += 1;
                // Estimate progress based on output lines (cap at 90%)
                on_progress(5.0 + (lines as f32 * 5.0).min(85.0));
            }
        }

        let status = child.wait()?;

        if !status.success() {
            let mut stderr_msg = String::new();
            if let Some(mut stderr) = child.stderr.take() {
                use std::io::Read;
                let _ = stderr.read_to_string(&mut stderr_msg);
            }
            return Err(format!(
                "brew {} failed (exit {:?}): {}",
                args.join(" "),
                status.code(),
                stderr_msg.trim()
            )
            .into());
        }

        on_progress(100.0);
        Ok(())
    }

    /// Create a Package for an installed cask
    fn cask_to_package(&self, cask: &BrewCask, version: &str) -> Package {
        let cache = &self.appstream_caches[0];

        Package {
            id: AppId::new(&format!("homebrew-cask-{}", cask.token)),
            icon: widget::icon::from_name("application-x-executable")
                .size(128)
                .handle(),
            info: Arc::new(AppInfo {
                source_id: cache.source_id.clone(),
                source_name: cache.source_name.clone(),
                name: cask
                    .name
                    .first()
                    .cloned()
                    .unwrap_or_else(|| cask.token.clone()),
                summary: cask.desc.clone().unwrap_or_default(),
                description: cask.desc.clone().unwrap_or_default(),
                pkgnames: vec![cask.token.clone()],
                urls: cask
                    .homepage
                    .as_ref()
                    .map(|h| vec![AppUrl::Homepage(h.clone())])
                    .unwrap_or_default(),
                ..Default::default()
            }),
            version: version.to_string(),
            extra: HashMap::new(),
        }
    }

    /// Create a Package for an outdated cask
    fn outdated_cask_to_package(&self, cask: &BrewOutdatedCask) -> Package {
        let cache = &self.appstream_caches[0];
        let mut extra = HashMap::new();
        extra.insert(
            format!("{}_installed", cask.name),
            cask.installed_versions.clone(),
        );
        extra.insert(
            format!("{}_update", cask.name),
            cask.current_version.clone(),
        );

        Package {
            id: AppId::new(&format!("homebrew-cask-{}", cask.name)),
            icon: widget::icon::from_name("application-x-executable")
                .size(128)
                .handle(),
            info: Arc::new(AppInfo {
                source_id: cache.source_id.clone(),
                source_name: cache.source_name.clone(),
                name: cask.name.clone(),
                summary: format!("{} → {}", cask.installed_versions, cask.current_version),
                description: String::new(),
                pkgnames: vec![cask.name.clone()],
                ..Default::default()
            }),
            version: cask.current_version.clone(),
            extra,
        }
    }

    /// Create a grouped system package entry for formulae
    fn create_formula_package(
        &self,
        formulae: Vec<(String, String, String)>, // (name, installed_version, update_version)
        is_update: bool,
    ) -> Package {
        let cache = &self.appstream_caches[0];
        let count = formulae.len();

        let mut description = String::new();
        let mut pkgnames = Vec::with_capacity(count);
        let mut extra = HashMap::new();

        for (name, installed, update) in formulae {
            if is_update {
                let _ = writeln!(description, " * {}: {} → {}", name, installed, update);
                extra.insert(format!("{}_installed", name), installed);
                extra.insert(format!("{}_update", name), update);
            } else {
                let _ = writeln!(description, " * {}: {}", name, installed);
                extra.insert(format!("{}_installed", name), installed);
            }
            pkgnames.push(name);
        }

        Package {
            id: AppId::system(),
            icon: widget::icon::from_name("package-x-generic")
                .size(128)
                .handle(),
            info: Arc::new(AppInfo {
                source_id: cache.source_id.clone(),
                source_name: cache.source_name.clone(),
                name: crate::fl!("homebrew-packages"),
                summary: crate::fl!("system-packages-summary", count = count),
                description,
                pkgnames,
                ..Default::default()
            }),
            version: String::new(),
            extra,
        }
    }

    /// Run an operation on a list of packages
    fn run_operation(
        &self,
        command: &str,
        packages: &[&str],
        is_cask: bool,
        on_progress: &mut dyn FnMut(f32),
        base_progress: f32,
        progress_share: f32,
    ) -> Result<(), Box<dyn Error>> {
        for (i, pkg) in packages.iter().enumerate() {
            let pkg_base = base_progress + (i as f32 / packages.len() as f32) * progress_share;
            let pkg_share = progress_share / packages.len() as f32;

            let args: Vec<&str> = if is_cask {
                vec![command, "--cask", pkg]
            } else {
                vec![command, pkg]
            };

            log::info!(
                "{} homebrew {} {}",
                command,
                if is_cask { "cask" } else { "formula" },
                pkg
            );

            self.run_brew_with_progress(&args, |p| {
                on_progress(pkg_base + (p / 100.0) * pkg_share);
            })?;
        }
        Ok(())
    }
}

impl Backend for Homebrew {
    fn load_caches(&mut self, refresh: bool) -> Result<(), Box<dyn Error>> {
        // Clear cache so next installed()/updates() call will refresh
        self.clear_cache();

        if refresh {
            log::info!("Refreshing Homebrew index");
            if let Err(e) = self.run_brew(&["update"]) {
                log::warn!("brew update failed: {}", e);
            }
        }
        Ok(())
    }

    fn info_caches(&self) -> &[AppstreamCache] {
        &self.appstream_caches
    }

    fn installed(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        // Ensure cache is populated (runs both brew commands in parallel on first call)
        self.ensure_cache()?;

        let cache = self.cache.lock().unwrap();
        let info = &cache.as_ref().unwrap().info;

        let mut packages = Vec::new();

        // Collect formulae
        let formulae: Vec<_> = info
            .formulae
            .iter()
            .map(|f| {
                let version = f
                    .installed
                    .first()
                    .map(|i| i.version.clone())
                    .unwrap_or_default();
                (f.full_name.clone(), version, String::new())
            })
            .collect();

        if !formulae.is_empty() {
            packages.push(self.create_formula_package(formulae, false));
        }

        // Add casks as individual entries
        for cask in &info.casks {
            let version = cask.installed.clone().unwrap_or_default();
            packages.push(self.cask_to_package(cask, &version));
        }

        Ok(packages)
    }

    fn updates(&self) -> Result<Vec<Package>, Box<dyn Error>> {
        // Ensure cache is populated (runs both brew commands in parallel on first call)
        self.ensure_cache()?;

        let cache = self.cache.lock().unwrap();
        let outdated = &cache.as_ref().unwrap().outdated;

        let mut packages = Vec::new();

        // Collect formula updates (skip pinned)
        let formulae: Vec<_> = outdated
            .formulae
            .iter()
            .filter(|f| !f.pinned)
            .map(|f| {
                let installed = f.installed_versions.first().cloned().unwrap_or_default();
                (f.name.clone(), installed, f.current_version.clone())
            })
            .collect();

        if !formulae.is_empty() {
            packages.push(self.create_formula_package(formulae, true));
        }

        // Add cask updates as individual entries
        for cask in &outdated.casks {
            packages.push(self.outdated_cask_to_package(cask));
        }

        Ok(packages)
    }

    fn file_packages(&self, _path: &str) -> Result<Vec<Package>, Box<dyn Error>> {
        Err("Homebrew does not support installing from files".into())
    }

    fn operation(
        &self,
        op: &Operation,
        mut f: Box<dyn FnMut(f32) + 'static>,
    ) -> Result<(), Box<dyn Error>> {
        // Separate casks and formulae based on each package's AppId
        let mut formulae = Vec::new();
        let mut casks = Vec::new();

        for (i, info) in op.infos.iter().enumerate() {
            // Check if this specific package is a cask based on its AppId
            let is_cask = op
                .package_ids
                .get(i)
                .is_some_and(|id| id.raw().starts_with("homebrew-cask-"));

            for pkgname in &info.pkgnames {
                if is_cask {
                    casks.push(pkgname.as_str());
                } else {
                    formulae.push(pkgname.as_str());
                }
            }
        }

        let total = formulae.len() + casks.len();
        if total == 0 {
            return Err("No packages specified".into());
        }

        let formula_share = (formulae.len() as f32 / total as f32) * 100.0;
        let cask_share = (casks.len() as f32 / total as f32) * 100.0;

        let command = match &op.kind {
            OperationKind::Install => "install",
            OperationKind::Uninstall { .. } => "uninstall",
            OperationKind::Update => "upgrade",
            OperationKind::RepositoryAdd(_) | OperationKind::RepositoryRemove(_, _) => {
                return Err("Homebrew does not support repository operations".into());
            }
        };

        // Run operations
        if !formulae.is_empty() {
            self.run_operation(command, &formulae, false, &mut *f, 0.0, formula_share)?;
        }

        if !casks.is_empty() {
            self.run_operation(command, &casks, true, &mut *f, formula_share, cask_share)?;
        }

        f(100.0);
        Ok(())
    }
}
