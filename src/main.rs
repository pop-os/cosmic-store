// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use clap::Parser;
use cosmic::{
    Application, ApplicationExt, Element, action,
    app::{Core, CosmicFlags, Settings, Task, context_drawer},
    cosmic_config::{self, CosmicConfigEntry},
    cosmic_theme, executor,
    iced::{
        Alignment, Length, Limits, Size, Subscription,
        core::SmolStr,
        event::{self, Event},
        futures::{self, SinkExt},
        keyboard::{Event as KeyEvent, Key, Modifiers},
        stream,
        widget::scrollable,
        window::{self, Event as WindowEvent},
    },
    theme,
    widget::{self},
};
use localize::LANGUAGE_SORTER;
use rayon::prelude::*;
use std::{
    any::TypeId,
    cell::Cell,
    cmp,
    collections::{BTreeMap, BTreeSet, HashMap, HashSet, VecDeque},
    fmt::Debug,
    future::pending,
    path::Path,
    sync::{Arc, Mutex},
    time::Instant,
};

use app_id::AppId;
mod app_id;

use app_info::{AppIcon, AppInfo, AppKind, AppProvide, AppUrl};
mod app_info;

use appstream_cache::AppstreamCache;
mod appstream_cache;

use backend::{BackendName, Backends, Package};
mod backend;

use config::{AppTheme, CONFIG_VERSION, Config};
mod config;

use editors_choice::EDITORS_CHOICE;
mod editors_choice;

use gstreamer::GStreamerCodec;
mod gstreamer;

mod icon_cache;

use key_bind::{KeyBind, key_binds};
mod key_bind;

mod localize;

#[cfg(feature = "logind")]
mod logind;

use operation::{Operation, OperationKind, RepositoryAdd, RepositoryRemove, RepositoryRemoveError};
mod operation;

use priority::priority;
mod priority;

mod stats;

use explore::ExplorePage;
mod explore;

use nav::{Category, CategoryIndex, NavPage, ScrollContext};
mod nav;

use search::{CachedExploreResults, SearchResult};
mod search;

mod view;

mod update;

pub const ICON_SIZE_SEARCH: u16 = 48;
pub const ICON_SIZE_PACKAGE: u16 = 64;
pub const ICON_SIZE_DETAILS: u16 = 128;
pub const MAX_GRID_WIDTH: f32 = 1600.0;
pub const MAX_RESULTS: usize = 100;

#[derive(Debug, Default, Parser)]
struct Cli {
    subcommand_opt: Option<String>,
    //TODO: should these extra gst-install-plugins-helper arguments actually be handled?
    #[arg(long)]
    transient_for: Option<String>,
    #[arg(long)]
    interaction: Option<String>,
    #[arg(long)]
    desktop_id: Option<String>,
    #[arg(long)]
    startup_notification_id: Option<String>,
}

/// Runs application with these settings
fn main() -> Result<(), Box<dyn std::error::Error>> {
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("warn"))
        .format_timestamp_millis()
        .init();

    localize::localize();

    let cli = Cli::parse();

    let (config_handler, config) = match cosmic_config::Config::new(App::APP_ID, CONFIG_VERSION) {
        Ok(config_handler) => {
            let config = match Config::get_entry(&config_handler) {
                Ok(ok) => ok,
                Err((errs, config)) => {
                    log::info!("errors loading config: {:?}", errs);
                    config
                }
            };
            (Some(config_handler), config)
        }
        Err(err) => {
            log::error!("failed to create config handler: {}", err);
            (None, Config::default())
        }
    };

    let mut settings = Settings::default();
    settings = settings.theme(config.app_theme.theme());
    settings = settings.size_limits(Limits::NONE.min_width(420.0).min_height(300.0));
    settings = settings.exit_on_close(false);

    let mut flags = Flags {
        subcommand_opt: cli.subcommand_opt,
        config_handler,
        config,
        mode: Mode::Normal,
    };

    if let Some(codec) = flags
        .subcommand_opt
        .as_ref()
        .and_then(|x| GStreamerCodec::parse(x))
    {
        // GStreamer installer dialog
        settings = settings.no_main_window(true);
        flags.mode = Mode::GStreamer {
            codec,
            selected: BTreeSet::new(),
            installing: false,
        };
        cosmic::app::run::<App>(settings, flags)?;
    } else {
        #[cfg(feature = "single-instance")]
        cosmic::app::run_single_instance::<App>(settings, flags)?;

        #[cfg(not(feature = "single-instance"))]
        cosmic::app::run::<App>(settings, flags)?;
    }

    Ok(())
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Action {
    SearchActivate,
    WindowClose,
}

impl Action {
    pub fn message(&self) -> Message {
        match self {
            Self::SearchActivate => Message::SearchActivate,
            Self::WindowClose => Message::WindowClose,
        }
    }
}

#[derive(Debug)]
pub struct AppEntry {
    pub backend_name: BackendName,
    pub info: Arc<AppInfo>,
    pub installed: bool,
}

pub type Apps = HashMap<AppId, Vec<AppEntry>>;

pub enum SourceKind {
    Recommended { data: &'static [u8], enabled: bool },
    Custom,
}

pub struct Source {
    pub backend_name: BackendName,
    pub id: String,
    pub name: String,
    pub kind: SourceKind,
    pub requires: Vec<String>,
}

impl Source {
    fn add(&self) -> Option<RepositoryAdd> {
        match self.kind {
            SourceKind::Recommended {
                data,
                enabled: false,
            } => Some(RepositoryAdd {
                id: self.id.clone(),
                data: data.to_vec(),
            }),
            _ => None,
        }
    }

    fn remove(&self) -> Option<RepositoryRemove> {
        match self.kind {
            SourceKind::Recommended { enabled: true, .. } | SourceKind::Custom => {
                Some(RepositoryRemove {
                    id: self.id.clone(),
                    name: self.name.clone(),
                })
            }
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
#[repr(i32)]
pub enum GStreamerExitCode {
    Success = 0,
    NotFound = 1,
    Error = 2,
    PartialSuccess = 3,
    UserAbort = 4,
}

#[derive(Clone, Debug)]
pub enum Mode {
    Normal,
    GStreamer {
        codec: GStreamerCodec,
        selected: BTreeSet<usize>,
        installing: bool,
    },
}

#[derive(Clone, Debug)]
pub struct Flags {
    subcommand_opt: Option<String>,
    config_handler: Option<cosmic_config::Config>,
    config: Config,
    mode: Mode,
}

//TODO
impl CosmicFlags for Flags {
    type SubCommand = String;
    type Args = Vec<String>;

    fn action(&self) -> Option<&String> {
        self.subcommand_opt.as_ref()
    }
}

/// Messages that are used specifically by our [`App`].
#[derive(Clone, Debug)]
pub enum Message {
    AppTheme(AppTheme),
    Backends(Backends),
    CategoryResults(&'static [Category], Vec<SearchResult>),
    CategoryIconsLoaded(&'static [Category], Vec<(usize, widget::icon::Handle)>),
    CheckUpdates,
    Config(Config),
    DialogCancel,
    DialogConfirm,
    DialogPage(DialogPage),
    ExplorePage(Option<ExplorePage>),
    AllExploreResults(
        HashMap<ExplorePage, Vec<SearchResult>>,
        CachedExploreResults,
    ),
    ExploreIconsLoaded(ExplorePage, Vec<(usize, widget::icon::Handle)>),
    ExploreCacheSaved(Result<(), String>),
    GStreamerExit(GStreamerExitCode),
    GStreamerInstall,
    GStreamerToggle(usize),
    AppsUpdated(Arc<Apps>, Arc<CategoryIndex>),
    Installed(Vec<(BackendName, Package)>),
    InstalledResults(Vec<SearchResult>),
    InstalledIconsLoaded(Vec<(usize, widget::icon::Handle)>),
    Key(Modifiers, Key, Option<SmolStr>),
    LaunchUrl(String),
    MaybeExit,
    #[cfg(feature = "notify")]
    Notification(Arc<Mutex<notify_rust::NotificationHandle>>),
    OpenDesktopId(String),
    Operation(OperationKind, BackendName, AppId, Arc<AppInfo>),
    PeriodicUpdateCheck,
    PendingComplete(u64),
    PendingDismiss,
    PendingError(u64, String),
    PendingProgress(u64, f32),
    RepositoryAdd(BackendName, Vec<RepositoryAdd>),
    RepositoryAddDialog(BackendName),
    RepositoryRemove(BackendName, Vec<RepositoryRemove>),
    ScrollView(scrollable::Viewport),
    SearchActivate,
    SearchClear,
    SearchInput(String),
    SearchResults(String, Vec<SearchResult>, bool),
    SearchIconsLoaded(String, Vec<(usize, widget::icon::Handle)>),
    SearchSubmit(String),
    Select(
        BackendName,
        AppId,
        Option<widget::icon::Handle>,
        Arc<AppInfo>,
    ),
    SelectInstalled(usize),
    SelectUpdates(usize),
    SelectNone,
    SelectCategoryResult(usize),
    SelectExploreResult(ExplorePage, usize),
    SelectSearchResult(usize),
    SelectedAddonsViewMore(bool),
    SelectedScreenshot(usize, String, Vec<u8>),
    SelectedScreenshotShown(usize),
    ToggleUninstallPurgeData(bool),
    SelectedSource(usize),
    SystemThemeModeChange(cosmic_theme::ThemeMode),
    ToggleContextPage(ContextPage),
    UpdateAll,
    Updates(Vec<(BackendName, Package)>),
    WindowClose,
    WindowNew,
    SelectPlacement(cosmic::widget::segmented_button::Entity),
    PlaceApplet(AppId),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ContextPage {
    Operations,
    ReleaseNotes(usize, String),
    Repositories,
    Settings,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DialogPage {
    FailedOperation(u64),
    RepositoryAddError(String),
    RepositoryRemove(BackendName, RepositoryRemoveError),
    Uninstall(BackendName, AppId, Arc<AppInfo>),
    Place(AppId),
}

#[derive(Clone, Debug)]
pub struct SelectedSource {
    pub backend_name: BackendName,
    pub source_id: String,
    pub source_name: String,
}

impl SelectedSource {
    pub fn new(backend_name: BackendName, info: &AppInfo, installed: bool) -> Self {
        SelectedSource {
            backend_name,
            source_id: info.source_id.clone(),
            source_name: if installed {
                fl!("source-installed", source = info.source_name.as_str())
            } else {
                info.source_name.clone()
            },
        }
    }
}

// For use in dropdown widget
impl AsRef<str> for SelectedSource {
    fn as_ref(&self) -> &str {
        &self.source_name
    }
}

#[derive(Clone, Debug)]
pub struct Selected {
    pub backend_name: BackendName,
    pub id: AppId,
    pub icon_opt: Option<widget::icon::Handle>,
    pub info: Arc<AppInfo>,
    pub screenshot_images: HashMap<usize, widget::image::Handle>,
    pub screenshot_shown: usize,
    pub sources: Vec<SelectedSource>,
    pub addons: Vec<(AppId, Arc<AppInfo>)>,
    pub addons_view_more: bool,
}

/// The [`App`] stores application-specific state.
pub struct App {
    pub core: Core,
    pub config_handler: Option<cosmic_config::Config>,
    pub config: Config,
    pub mode: Mode,
    pub locale: String,
    pub app_themes: Vec<String>,
    pub apps: Arc<Apps>,
    pub category_index: Arc<CategoryIndex>,
    pub backends: Backends,
    pub context_page: ContextPage,
    pub dialog_pages: VecDeque<DialogPage>,
    pub explore_page_opt: Option<ExplorePage>,
    pub key_binds: HashMap<KeyBind, Action>,
    pub nav_model: widget::nav_bar::Model,
    #[cfg(feature = "notify")]
    pub notification_opt: Option<Arc<Mutex<notify_rust::NotificationHandle>>>,
    pub pending_operation_id: u64,
    pub pending_operations: BTreeMap<u64, (Operation, f32)>,
    pub progress_operations: BTreeSet<u64>,
    pub complete_operations: BTreeMap<u64, Operation>,
    pub failed_operations: BTreeMap<u64, (Operation, f32, String)>,
    pub repos_changing: Vec<(BackendName, String, bool)>,
    pub scrollable_id: widget::Id,
    pub scroll_views: HashMap<ScrollContext, scrollable::Viewport>,
    pub search_active: bool,
    pub search_id: widget::Id,
    pub search_input: String,
    pub size: Cell<Option<Size>>,
    //TODO: use hashset?
    pub installed: Option<Vec<(BackendName, Package)>>,
    //TODO: use hashset?
    pub updates: Option<Vec<(BackendName, Package)>>,
    //TODO: use hashset?
    pub waiting_installed: Vec<(BackendName, String, AppId)>,
    //TODO: use hashset?
    pub waiting_updates: Vec<(BackendName, String, AppId)>,
    pub category_results: Option<(&'static [Category], Vec<SearchResult>)>,
    pub category_load_start: Option<Instant>,
    pub explore_results: HashMap<ExplorePage, Vec<SearchResult>>,
    pub explore_load_start: Option<Instant>,
    pub installed_results: Option<Vec<SearchResult>>,
    pub search_results: Option<(String, Vec<SearchResult>)>,
    pub selected_opt: Option<Selected>,
    pub applet_placement_buttons: cosmic::widget::segmented_button::SingleSelectModel,
    pub uninstall_purge_data: bool,
}

impl App {
    fn open_desktop_id(&self, mut desktop_id: String) -> Task<Message> {
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    if !desktop_id.ends_with(".desktop") {
                        desktop_id.push_str(".desktop");
                    }
                    let xdg_dirs = xdg::BaseDirectories::with_prefix("applications");
                    let path = match xdg_dirs.find_data_file(&desktop_id) {
                        Some(some) => some,
                        None => {
                            log::warn!("failed to find desktop file for {:?}", desktop_id);
                            return None;
                        }
                    };
                    let entry = match freedesktop_entry_parser::parse_entry(&path) {
                        Ok(ok) => ok,
                        Err(err) => {
                            log::warn!("failed to read desktop file {:?}: {}", path, err);
                            return None;
                        }
                    };
                    //TODO: handle Terminal=true
                    let Some(exec) = entry
                        .get("Desktop Entry", "Exec")
                        .and_then(|attr| attr.first())
                    else {
                        log::warn!("no exec section in {:?}", path);
                        return None;
                    };
                    //TODO: use libcosmic for loading desktop data
                    Some((exec.to_string(), desktop_id))
                })
                .await
                .unwrap_or(None)
            },
            |result| {
                #[cfg(feature = "desktop")]
                if let Some((exec, desktop_id)) = result {
                    tokio::spawn(async move {
                        cosmic::desktop::spawn_desktop_exec(
                            &exec,
                            Vec::<(&str, &str)>::new(),
                            Some(&desktop_id),
                            false,
                        )
                        .await;
                    });
                }
                action::none()
            },
        )
    }

    fn operation(&mut self, operation: Operation) {
        match &operation.kind {
            OperationKind::RepositoryAdd(adds) => {
                for add in adds.iter() {
                    self.repos_changing
                        .push((operation.backend_name, add.id.clone(), true));
                }
            }
            OperationKind::RepositoryRemove(rms, _) => {
                for rm in rms.iter() {
                    self.repos_changing
                        .push((operation.backend_name, rm.id.clone(), false));
                }
            }
            _ => {}
        }

        let id = self.pending_operation_id;
        self.pending_operation_id += 1;
        self.progress_operations.insert(id);
        self.pending_operations.insert(id, (operation, 0.0));
    }

    fn generic_search<F: Fn(&AppId, &AppInfo, bool) -> Option<i64> + Send + Sync>(
        apps: &Apps,
        backends: &Backends,
        filter_map: F,
    ) -> Vec<SearchResult> {
        let filter_start = Instant::now();
        let num_apps = apps.len();
        let mut results: Vec<SearchResult> = apps
            .par_iter()
            .filter_map(|(id, infos)| {
                let mut best_weight: Option<i64> = None;
                for AppEntry {
                    backend_name,
                    info,
                    installed,
                } in infos.iter()
                {
                    if let Some(weight) = filter_map(id, info, *installed) {
                        // Skip if best weight has equal or lower weight
                        if let Some(prev_weight) = best_weight {
                            if prev_weight <= weight {
                                continue;
                            }
                        }

                        // Replace best weight
                        best_weight = Some(weight);
                    }
                }
                let weight = best_weight?;
                // Use first info as it is preferred, even if other ones had a higher weight
                let AppEntry {
                    backend_name,
                    info,
                    installed,
                } = infos.first()?;
                Some(SearchResult {
                    backend_name: *backend_name,
                    id: id.clone(),
                    icon_opt: None,
                    info: info.clone(),
                    weight,
                })
            })
            .collect();
        results.par_sort_unstable_by(|a, b| match a.weight.cmp(&b.weight) {
            cmp::Ordering::Equal => match LANGUAGE_SORTER.compare(&a.info.name, &b.info.name) {
                cmp::Ordering::Equal => a.backend_name.cmp(&b.backend_name),
                ordering => ordering,
            },
            ordering => ordering,
        });
        log::debug!(
            "generic_search: scanned {} apps in {:?}",
            num_apps,
            filter_start.elapsed()
        );
        // Load only enough icons to show one page of results
        //TODO: load in background
        for result in results.iter_mut().take(MAX_RESULTS) {
            let Some(backend) = backends.get(&result.backend_name) else {
                continue;
            };
            let appstream_caches = backend.info_caches();
            let Some(appstream_cache) = appstream_caches
                .iter()
                .find(|x| x.source_id == result.info.source_id)
            else {
                continue;
            };
            result.icon_opt = Some(appstream_cache.icon(&result.info));
        }
        results
    }

    /// Fast category search using pre-built index - O(results) instead of O(all_apps)
    fn category_search_indexed(
        apps: &Apps,
        backends: &Backends,
        category_index: &CategoryIndex,
        categories: &[Category],
    ) -> Vec<SearchResult> {
        let filter_start = Instant::now();

        // Collect all app IDs matching any of the categories
        let mut matching_ids: HashSet<&AppId> = HashSet::new();
        for category in categories {
            if let Some(ids) = category_index.get(category.id()) {
                matching_ids.extend(ids.iter());
            }
        }

        // Process only matching apps (all indexed apps are already DesktopApplications)
        let mut results: Vec<SearchResult> = matching_ids
            .par_iter()
            .filter_map(|id| {
                let entries = apps.get(*id)?;
                let AppEntry {
                    backend_name,
                    info,
                    installed: _,
                } = entries.first()?;

                Some(SearchResult {
                    backend_name: *backend_name,
                    id: (*id).clone(),
                    icon_opt: None,
                    info: info.clone(),
                    weight: -(info.monthly_downloads as i64),
                })
            })
            .collect();

        // Sort by weight (monthly downloads), then by name
        results.par_sort_unstable_by(|a, b| match a.weight.cmp(&b.weight) {
            cmp::Ordering::Equal => match LANGUAGE_SORTER.compare(&a.info.name, &b.info.name) {
                cmp::Ordering::Equal => a.backend_name.cmp(&b.backend_name),
                ordering => ordering,
            },
            ordering => ordering,
        });

        log::debug!(
            "category_search_indexed: looked up {} ids in {:?}",
            results.len(),
            filter_start.elapsed()
        );

        // Load icons for top results
        for result in results.iter_mut().take(MAX_RESULTS) {
            let Some(backend) = backends.get(&result.backend_name) else {
                continue;
            };
            let appstream_caches = backend.info_caches();
            let Some(appstream_cache) = appstream_caches
                .iter()
                .find(|x| x.source_id == result.info.source_id)
            else {
                continue;
            };
            result.icon_opt = Some(appstream_cache.icon(&result.info));
        }
        results
    }

    fn categories(&self, categories: &'static [Category]) -> Task<Message> {
        let apps = self.apps.clone();
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let applet_provide = AppProvide::Id("com.system76.CosmicApplet".to_string());
                    let results =
                        Self::generic_search(&apps, &backends, |_id, info, _installed| {
                            if !matches!(info.kind, AppKind::DesktopApplication) {
                                return None;
                            }
                            for category in categories {
                                //TODO: this hack makes it easier to add applets to the nav bar
                                if matches!(category, Category::CosmicApplet) {
                                    if info.provides.contains(&applet_provide) {
                                        return Some(-(info.monthly_downloads as i64));
                                    }
                                } else {
                                    //TODO: contains doesn't work due to type mismatch
                                    if info.categories.iter().any(|x| x == category.id()) {
                                        return Some(-(info.monthly_downloads as i64));
                                    }
                                }
                            }
                            None
                        });
                    let duration = start.elapsed();
                    log::info!(
                        "searched for categories {:?} in {:?}, found {} results",
                        categories,
                        duration,
                        results.len()
                    );
                    action::app(Message::CategoryResults(categories, results))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn search_explore_page(
        explore_page: ExplorePage,
        apps: &Apps,
        backends: &Backends,
        category_index: &CategoryIndex,
    ) -> Vec<SearchResult> {
        let now = chrono::Utc::now().timestamp();
        match explore_page {
            ExplorePage::EditorsChoice => {
                Self::generic_search(apps, backends, |id, _info, _installed| {
                    EDITORS_CHOICE
                        .iter()
                        .position(|choice_id| choice_id == &id.normalized())
                        .map(|x| x as i64)
                })
            }
            ExplorePage::PopularApps => {
                Self::generic_search(apps, backends, |_id, info, _installed| {
                    if !matches!(info.kind, AppKind::DesktopApplication) {
                        return None;
                    }
                    Some(-(info.monthly_downloads as i64))
                })
            }
            ExplorePage::MadeForCosmic => {
                let provide = AppProvide::Id("com.system76.CosmicApplication".to_string());
                Self::generic_search(apps, backends, |_id, info, _installed| {
                    if !matches!(info.kind, AppKind::DesktopApplication) {
                        return None;
                    }
                    if info.provides.contains(&provide) {
                        Some(-(info.monthly_downloads as i64))
                    } else {
                        None
                    }
                })
            }
            ExplorePage::NewApps => {
                Self::generic_search(apps, backends, |_id, _info, _installed| {
                    //TODO
                    None
                })
            }
            ExplorePage::RecentlyUpdated => {
                Self::generic_search(apps, backends, |id, info, _installed| {
                    if !matches!(info.kind, AppKind::DesktopApplication) {
                        return None;
                    }
                    // Finds the newest release and sorts from newest to oldest
                    //TODO: appstream release info is often incomplete
                    let mut min_weight = 0;
                    for release in info.releases.iter() {
                        if let Some(timestamp) = release.timestamp {
                            if timestamp < now {
                                let weight = -timestamp;
                                if weight < min_weight {
                                    min_weight = weight;
                                }
                            } else {
                                log::info!(
                                    "{:?} has release timestamp {} which is past the present {}",
                                    id,
                                    timestamp,
                                    now
                                );
                            }
                        }
                    }
                    Some(min_weight)
                })
            }
            _ => {
                let categories = explore_page.categories();
                Self::category_search_indexed(apps, backends, category_index, categories)
            }
        }
    }

    fn explore_results_all(&self) -> Task<Message> {
        let apps = self.apps.clone();
        let backends = self.backends.clone();
        let category_index = self.category_index.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    log::info!("start batched search for all explore pages");
                    let overall_start = Instant::now();
                    let mut all_results = HashMap::new();
                    for &explore_page in ExplorePage::all() {
                        let start = Instant::now();
                        let results = Self::search_explore_page(
                            explore_page,
                            &apps,
                            &backends,
                            &category_index,
                        );
                        log::info!(
                            "searched for {:?} in {:?}, found {} results",
                            explore_page,
                            start.elapsed(),
                            results.len()
                        );
                        all_results.insert(explore_page, results);
                    }
                    let cached = CachedExploreResults::from_results(&all_results, &backends);
                    log::info!(
                        "batched explore search completed in {:?}",
                        overall_start.elapsed()
                    );
                    action::app(Message::AllExploreResults(all_results, cached))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn load_explore_icons(&self, explore_page: ExplorePage) -> Task<Message> {
        let results = match self.explore_results.get(&explore_page) {
            Some(results) => results.clone(),
            None => return Task::none(),
        };
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let icons = Self::load_icons_for_results(&results, &backends);
                    action::app(Message::ExploreIconsLoaded(explore_page, icons))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn load_category_icons(&self, categories: &'static [Category]) -> Task<Message> {
        let results = match &self.category_results {
            Some((cats, results)) if *cats == categories => results.clone(),
            _ => return Task::none(),
        };
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let icons = Self::load_icons_for_results(&results, &backends);
                    action::app(Message::CategoryIconsLoaded(categories, icons))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn load_installed_icons(&self) -> Task<Message> {
        let results = match &self.installed_results {
            Some(results) => results.clone(),
            None => return Task::none(),
        };
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let icons = Self::load_icons_for_results(&results, &backends);
                    action::app(Message::InstalledIconsLoaded(icons))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn load_search_icons(&self, input: String) -> Task<Message> {
        let results = match &self.search_results {
            Some((query, results)) if *query == input => results.clone(),
            _ => return Task::none(),
        };
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let icons = Self::load_icons_for_results(&results, &backends);
                    action::app(Message::SearchIconsLoaded(input, icons))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn load_icons_for_results(
        results: &[SearchResult],
        backends: &Backends,
    ) -> Vec<(usize, widget::icon::Handle)> {
        let icon_start = Instant::now();
        let mut icons = Vec::new();
        for (i, result) in results.iter().enumerate().take(MAX_RESULTS) {
            // Skip results that already have icons (e.g., preserved from previous results)
            if result.icon_opt.is_some() {
                continue;
            }
            let Some(backend) = backends.get(&result.backend_name) else {
                continue;
            };
            let appstream_caches = backend.info_caches();
            let Some(appstream_cache) = appstream_caches
                .iter()
                .find(|x| x.source_id == result.info.source_id)
            else {
                continue;
            };
            icons.push((i, appstream_cache.icon(&result.info)));
        }
        log::debug!(
            "icon loading: loaded {} icons in {:?} (background)",
            icons.len(),
            icon_start.elapsed()
        );
        icons
    }

    fn installed_results(&self) -> Task<Message> {
        let apps = self.apps.clone();
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let results = Self::generic_search(&apps, &backends, |id, _info, installed| {
                        if installed {
                            Some(if id.is_system() { -1 } else { 0 })
                        } else {
                            None
                        }
                    });
                    let duration = start.elapsed();
                    log::info!(
                        "searched for installed in {:?}, found {} results",
                        duration,
                        results.len()
                    );
                    action::app(Message::InstalledResults(results))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn search(&self) -> Task<Message> {
        let input = self.search_input.clone();

        // Handle supported URI schemes before trying plain text search
        if let Ok(url) = reqwest::Url::parse(&input) {
            match url.scheme() {
                "appstream" => {
                    return self.handle_appstream_url(input, url.path());
                }
                "file" => {
                    return self.handle_file_url(input, url.path());
                }
                "mime" => {
                    // This is a workaround to be able to search for mime handlers, mime is not a real URL scheme
                    return self.handle_mime_url(input, url.path());
                }
                scheme => {
                    log::warn!("unsupported URL scheme {scheme} in {url}");
                }
            }
        }

        // Also handle standard file paths
        if input.starts_with("/") && Path::new(&input).is_file() {
            return self.handle_file_url(input.clone(), &input);
        }

        // Also handle gstreamer codec strings
        if let Some(gstreamer_codec) = GStreamerCodec::parse(&input) {
            return self.handle_gstreamer_codec(input.clone(), gstreamer_codec);
        }

        let pattern = regex::escape(&input);
        let regex = match regex::RegexBuilder::new(&pattern)
            .case_insensitive(true)
            .build()
        {
            Ok(ok) => ok,
            Err(err) => {
                log::warn!("failed to parse regex {:?}: {}", pattern, err);
                return Task::none();
            }
        };
        let apps = self.apps.clone();
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let results = Self::generic_search(&apps, &backends, |id, info, _installed| {
                        if !matches!(info.kind, AppKind::DesktopApplication) {
                            return None;
                        }
                        //TODO: improve performance
                        let stats_weight = |weight: i64| -> i64 {
                            //TODO: make sure no overflows
                            (weight << 56) - (info.monthly_downloads as i64)
                        };

                        //TODO: fuzzy match (nucleus-matcher?)
                        let regex_weight = |string: &str, weight: i64| -> Option<i64> {
                            let mat = regex.find(string)?;
                            if mat.range().start == 0 {
                                if mat.range().end == string.len() {
                                    // String equals search phrase
                                    Some(stats_weight(weight + 0))
                                } else {
                                    // String starts with search phrase
                                    Some(stats_weight(weight + 1))
                                }
                            } else {
                                // String contains search phrase
                                Some(stats_weight(weight + 2))
                            }
                        };
                        if let Some(weight) = regex_weight(&info.name, 0) {
                            return Some(weight);
                        }
                        if let Some(weight) = regex_weight(&info.summary, 3) {
                            return Some(weight);
                        }
                        if let Some(weight) = regex_weight(&info.description, 6) {
                            return Some(weight);
                        }
                        None
                    });
                    let duration = start.elapsed();
                    log::info!(
                        "searched for {:?} in {:?}, found {} results",
                        input,
                        duration,
                        results.len()
                    );
                    action::app(Message::SearchResults(input, results, false))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn select(
        &mut self,
        backend_name: BackendName,
        id: AppId,
        icon_opt: Option<widget::icon::Handle>,
        info: Arc<AppInfo>,
    ) -> Task<Message> {
        log::info!(
            "selected {:?} from backend {:?} and source {:?}",
            id,
            backend_name,
            info.source_id
        );
        let sources = self.selected_sources(backend_name, &id, &info);
        let addons = self.selected_addons(backend_name, &id, &info);
        self.selected_opt = Some(Selected {
            backend_name,
            id,
            icon_opt,
            info,
            screenshot_images: HashMap::new(),
            screenshot_shown: 0,
            sources,
            addons,
            addons_view_more: false,
        });
        self.update_scroll()
    }

    fn scroll_context(&self) -> ScrollContext {
        if self.selected_opt.is_some() {
            ScrollContext::Selected
        } else if self.search_results.is_some() {
            ScrollContext::SearchResults
        } else if self.explore_page_opt.is_some() {
            ScrollContext::ExplorePage
        } else {
            ScrollContext::NavPage
        }
    }

    fn update_scroll(&mut self) -> Task<Message> {
        let scroll_context = self.scroll_context();
        // Clear unused scroll contexts
        for remove_context in scroll_context.unused_contexts() {
            self.scroll_views.remove(remove_context);
        }
        scrollable::scroll_to(
            self.scrollable_id.clone(),
            match self.scroll_views.get(&scroll_context) {
                Some(viewport) => viewport.absolute_offset(),
                None => scrollable::AbsoluteOffset::default(),
            },
        )
    }

    fn update_backends(&mut self, refresh: bool) -> Task<Message> {
        let locale = self.locale.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let backends = backend::backends(&locale, refresh);
                    let duration = start.elapsed();
                    log::info!(
                        "loaded backends {} in {:?}",
                        if refresh {
                            "with refreshing"
                        } else {
                            "without refreshing"
                        },
                        duration
                    );
                    action::app(Message::Backends(backends))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn update_config(&mut self) -> Task<Message> {
        cosmic::command::set_theme(self.config.app_theme.theme())
    }

    fn is_installed_inner(
        installed_opt: &Option<Vec<(BackendName, Package)>>,
        backend_name: BackendName,
        id: &AppId,
        info: &AppInfo,
    ) -> bool {
        if let Some(installed) = installed_opt {
            for (installed_backend_name, package) in installed {
                if *installed_backend_name == backend_name
                    && package.info.source_id == info.source_id
                {
                    // Simple app match found
                    if &package.id == id {
                        return true;
                    }

                    // Search for matching pkgnames
                    //TODO: also do flatpak refs?
                    if package.id.is_system() && !info.pkgnames.is_empty() {
                        let mut found = true;
                        for pkgname in info.pkgnames.iter() {
                            if !package.info.pkgnames.contains(pkgname) {
                                found = false;
                                break;
                            }
                        }
                        if found {
                            return true;
                        }
                    }
                }
            }
        }
        false
    }

    pub fn is_installed(&self, backend_name: BackendName, id: &AppId, info: &AppInfo) -> bool {
        Self::is_installed_inner(&self.installed, backend_name, id, info)
    }

    fn update_apps(&self) -> Task<Message> {
        let backends = self.backends.clone();
        let installed = self.installed.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let mut apps = Apps::new();

                    let entry_sort = |a: &AppEntry, b: &AppEntry, id: &AppId| {
                        // Sort with installed first
                        match b.installed.cmp(&a.installed) {
                            cmp::Ordering::Equal => {
                                // Sort by highest priority first to lowest priority
                                let a_priority = priority(a.backend_name, &a.info.source_id, id);
                                let b_priority = priority(b.backend_name, &b.info.source_id, id);
                                match b_priority.cmp(&a_priority) {
                                    cmp::Ordering::Equal => {
                                        match LANGUAGE_SORTER
                                            .compare(&a.info.source_id, &b.info.source_id)
                                        {
                                            cmp::Ordering::Equal => {
                                                a.backend_name.cmp(&b.backend_name)
                                            }
                                            ordering => ordering,
                                        }
                                    }
                                    ordering => ordering,
                                }
                            }
                            ordering => ordering,
                        }
                    };

                    // Collect all entries from backends in parallel
                    let collect_start = Instant::now();
                    let installed_ref = &installed;
                    let all_entries: Vec<(AppId, AppEntry)> = backends
                        .par_iter()
                        .flat_map(|(backend_name, backend)| {
                            backend
                                .info_caches()
                                .iter()
                                .flat_map(|appstream_cache| {
                                    appstream_cache.infos.iter().map(|(id, info)| {
                                        (
                                            id.clone(),
                                            AppEntry {
                                                backend_name: *backend_name,
                                                info: info.clone(),
                                                installed: Self::is_installed_inner(
                                                    installed_ref,
                                                    *backend_name,
                                                    id,
                                                    info,
                                                ),
                                            },
                                        )
                                    })
                                })
                                .collect::<Vec<_>>()
                        })
                        .collect();

                    let entry_count = all_entries.len();

                    // Merge entries into HashMap
                    for (id, entry) in all_entries {
                        apps.entry(id).or_default().push(entry);
                    }
                    log::debug!(
                        "update_apps: collected {} entries in {:?}",
                        entry_count,
                        collect_start.elapsed()
                    );

                    // Manually insert system apps
                    if let Some(installed) = &installed {
                        for (backend_name, package) in installed {
                            if package.id.is_system() {
                                apps.entry(package.id.clone()).or_default().push(AppEntry {
                                    backend_name: *backend_name,
                                    info: package.info.clone(),
                                    installed: true,
                                });
                            }
                        }
                    }

                    // Sort all entries once at the end (in parallel)
                    let sort_start = Instant::now();
                    apps.par_iter_mut().for_each(|(id, entries)| {
                        entries.sort_unstable_by(|a, b| entry_sort(a, b, id));
                    });
                    log::debug!("update_apps: sorted entries in {:?}", sort_start.elapsed());

                    // Build category index for fast category lookups (only desktop apps)
                    let mut category_index = CategoryIndex::new();
                    for (id, entries) in apps.iter() {
                        // Use the first entry (highest priority) for category indexing
                        if let Some(entry) = entries.first() {
                            // Only index desktop applications
                            if matches!(entry.info.kind, AppKind::DesktopApplication) {
                                for category in &entry.info.categories {
                                    category_index
                                        .entry(category.clone())
                                        .or_default()
                                        .push(id.clone());
                                }
                            }
                        }
                    }

                    let duration = start.elapsed();
                    log::info!(
                        "update_apps: built app cache with {} ids in {:?}",
                        apps.len(),
                        duration
                    );

                    action::app(Message::AppsUpdated(
                        Arc::new(apps),
                        Arc::new(category_index),
                    ))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn update_installed(&self) -> Task<Message> {
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let total_start = Instant::now();
                    let collect_start = Instant::now();
                    let mut installed: Vec<_> = backends
                        .par_iter()
                        .flat_map(|(backend_name, backend)| {
                            let start = Instant::now();
                            let result: Vec<_> = match backend.installed() {
                                Ok(packages) => packages
                                    .into_iter()
                                    .map(|package| (*backend_name, package))
                                    .collect(),
                                Err(err) => {
                                    log::error!("failed to list installed: {}", err);
                                    Vec::new()
                                }
                            };
                            let duration = start.elapsed();
                            log::info!("loaded installed from {} in {:?}", backend_name, duration);
                            result
                        })
                        .collect();
                    log::debug!(
                        "update_installed: collected {} packages in {:?}",
                        installed.len(),
                        collect_start.elapsed()
                    );
                    let sort_start = Instant::now();
                    installed.par_sort_unstable_by(|a, b| {
                        let a_is_system = a.1.id.is_system();
                        let b_is_system = b.1.id.is_system();
                        if a_is_system && !b_is_system {
                            cmp::Ordering::Less
                        } else if b_is_system && !a_is_system {
                            cmp::Ordering::Greater
                        } else {
                            LANGUAGE_SORTER.compare(&a.1.info.name, &b.1.info.name)
                        }
                    });
                    log::debug!(
                        "update_installed: sorted in {:?}, total {:?}",
                        sort_start.elapsed(),
                        total_start.elapsed()
                    );
                    action::app(Message::Installed(installed))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn update_updates(&self) -> Task<Message> {
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let total_start = Instant::now();
                    let collect_start = Instant::now();
                    let mut updates: Vec<_> = backends
                        .par_iter()
                        .flat_map(|(backend_name, backend)| {
                            let start = Instant::now();
                            let result: Vec<_> = match backend.updates() {
                                Ok(packages) => packages
                                    .into_iter()
                                    .map(|package| (*backend_name, package))
                                    .collect(),
                                Err(err) => {
                                    log::error!("failed to list updates: {}", err);
                                    Vec::new()
                                }
                            };
                            let duration = start.elapsed();
                            log::info!("loaded updates from {} in {:?}", backend_name, duration);
                            result
                        })
                        .collect();
                    log::debug!(
                        "update_updates: collected {} packages in {:?}",
                        updates.len(),
                        collect_start.elapsed()
                    );
                    let sort_start = Instant::now();
                    updates.par_sort_unstable_by(|a, b| {
                        if a.1.id.is_system() {
                            cmp::Ordering::Less
                        } else if b.1.id.is_system() {
                            cmp::Ordering::Greater
                        } else {
                            LANGUAGE_SORTER.compare(&a.1.info.name, &b.1.info.name)
                        }
                    });
                    log::debug!(
                        "update_updates: sorted in {:?}, total {:?}",
                        sort_start.elapsed(),
                        total_start.elapsed()
                    );
                    action::app(Message::Updates(updates))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn update_notification(&mut self) -> Task<Message> {
        // Handle closing notification if there are no operations
        if self.pending_operations.is_empty() {
            #[cfg(feature = "notify")]
            if let Some(notification_arc) = self.notification_opt.take() {
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            //TODO: this is nasty
                            let notification_mutex = Arc::try_unwrap(notification_arc).unwrap();
                            let notification = notification_mutex.into_inner().unwrap();
                            notification.close();
                        })
                        .await
                        .unwrap();
                        action::app(Message::MaybeExit)
                    },
                    |x| x,
                );
            }
        }

        Task::none()
    }

    fn handle_appstream_url(&self, input: String, path: &str) -> Task<Message> {
        // Handler for appstream:component-id as described in:
        // https://freedesktop.org/software/appstream/docs/sect-AppStream-Misc-URIHandler.html
        let apps = self.apps.clone();
        let backends = self.backends.clone();
        let component_id = AppId::new(path.trim_start_matches('/'));
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let results =
                        Self::generic_search(&apps, &backends, |id, _info, _installed| {
                            //TODO: fuzzy search with lower weight?
                            if id == &component_id { Some(0) } else { None }
                        });
                    let duration = start.elapsed();
                    log::info!(
                        "searched for ID {:?} in {:?}, found {} results",
                        component_id,
                        duration,
                        results.len()
                    );
                    action::app(Message::SearchResults(input, results, true))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn handle_file_url(&self, input: String, path: &str) -> Task<Message> {
        let path = path.to_string();
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let mut packages = Vec::new();
                    for (backend_name, backend) in backends.iter() {
                        match backend.file_packages(&path) {
                            Ok(backend_packages) => {
                                for package in backend_packages {
                                    packages.push((backend_name, package));
                                }
                            }
                            Err(err) => {
                                log::warn!(
                                    "failed to load file {:?} using backend {:?}: {}",
                                    path,
                                    backend_name,
                                    err
                                );
                            }
                        }
                    }
                    let duration = start.elapsed();
                    log::info!(
                        "loaded file {:?} in {:?}, found {} packages",
                        path,
                        duration,
                        packages.len()
                    );

                    //TODO: store the resolved packages somewhere
                    let mut results = Vec::with_capacity(packages.len());
                    for (backend_name, package) in packages {
                        results.push(SearchResult {
                            backend_name: *backend_name,
                            id: package.id,
                            icon_opt: Some(package.icon),
                            info: package.info,
                            weight: 0,
                        });
                    }
                    action::app(Message::SearchResults(input, results, true))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn handle_gstreamer_codec(
        &self,
        input: String,
        gstreamer_codec: GStreamerCodec,
    ) -> Task<Message> {
        let backends = self.backends.clone();
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let mut packages = Vec::new();
                    for (backend_name, backend) in backends.iter() {
                        match backend.gstreamer_packages(&gstreamer_codec) {
                            Ok(backend_packages) => {
                                for package in backend_packages {
                                    packages.push((backend_name, package));
                                }
                            }
                            Err(err) => {
                                log::warn!(
                                    "failed to load gstreamer codec {:?} using backend {:?}: {}",
                                    gstreamer_codec,
                                    backend_name,
                                    err
                                );
                            }
                        }
                    }
                    let duration = start.elapsed();
                    log::info!(
                        "loaded gstreamer codec {:?} in {:?}, found {} packages",
                        gstreamer_codec,
                        duration,
                        packages.len()
                    );

                    //TODO: store the resolved packages somewhere
                    let mut results = Vec::with_capacity(packages.len());
                    for (backend_name, package) in packages {
                        results.push(SearchResult {
                            backend_name: *backend_name,
                            id: package.id,
                            icon_opt: Some(package.icon),
                            info: package.info,
                            weight: 0,
                        });
                    }
                    action::app(Message::SearchResults(input, results, true))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn handle_mime_url(&self, input: String, path: &str) -> Task<Message> {
        let apps = self.apps.clone();
        let backends = self.backends.clone();
        let mime = path.trim_matches('/').to_string();
        let provide = AppProvide::MediaType(mime.clone());
        Task::perform(
            async move {
                tokio::task::spawn_blocking(move || {
                    let start = Instant::now();
                    let results =
                        Self::generic_search(&apps, &backends, |_id, info, _installed| {
                            //TODO: monthly downloads as weight?
                            if info.provides.contains(&provide) {
                                Some(-(info.monthly_downloads as i64))
                            } else {
                                None
                            }
                        });
                    let duration = start.elapsed();
                    log::info!(
                        "searched for mime {:?} in {:?}, found {} results",
                        mime,
                        duration,
                        results.len()
                    );
                    action::app(Message::SearchResults(input, results, false))
                })
                .await
                .unwrap_or(action::none())
            },
            |x| x,
        )
    }

    fn update_title(&mut self) -> Task<Message> {
        if let Some(window_id) = &self.core.main_window_id() {
            self.set_window_title(fl!("app-name"), *window_id)
        } else {
            Task::none()
        }
    }

    fn operations(&self) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_xs, space_m, ..
        } = theme::active().cosmic().spacing;

        let mut children = Vec::new();

        //TODO: get height from theme?
        let progress_bar_height = Length::Fixed(4.0);

        if !self.pending_operations.is_empty() {
            let mut section = widget::settings::section().title(fl!("pending"));
            for (_id, (op, progress)) in self.pending_operations.iter().rev() {
                section = section.add(widget::column::with_children(vec![
                    widget::progress_bar(0.0..=100.0, *progress)
                        .height(progress_bar_height)
                        .into(),
                    widget::Space::with_height(space_xs).into(),
                    widget::text(op.pending_text(*progress as i32)).into(),
                ]));
            }
            children.push(section.into());
        }

        if !self.failed_operations.is_empty() {
            let mut section = widget::settings::section().title(fl!("failed"));
            for (_id, (op, progress, error)) in self.failed_operations.iter().rev() {
                section = section.add(widget::column::with_children(vec![
                    widget::text(op.pending_text(*progress as i32)).into(),
                    widget::text(error).into(),
                ]));
            }
            children.push(section.into());
        }

        if !self.complete_operations.is_empty() {
            let mut section = widget::settings::section().title(fl!("complete"));
            for (_id, op) in self.complete_operations.iter().rev() {
                section = section.add(widget::text(op.completed_text()));
            }
            children.push(section.into());
        }

        if children.is_empty() {
            children.push(widget::text::body(fl!("no-operations")).into());
        }

        widget::column::with_children(children)
            .spacing(space_m)
            .into()
    }

    fn release_notes(&self, index: usize) -> Element<'_, Message> {
        let cosmic_theme::Spacing {
            space_s, space_xxs, ..
        } = theme::active().cosmic().spacing;

        // Check if this is a system package update
        if let Some(package) = self
            .updates
            .as_deref()
            .and_then(|updates| updates.get(index).map(|(_, package)| package))
        {
            if package.id.is_system() {
                // Use pkgnames for most backends, flatpak_refs for flatpak
                let refs: Vec<&str> = if !package.info.pkgnames.is_empty() {
                    package.info.pkgnames.iter().map(|s| s.as_str()).collect()
                } else {
                    package
                        .info
                        .flatpak_refs
                        .iter()
                        .map(|s| s.as_str())
                        .collect()
                };

                // Display list of system packages with version info
                let mut package_list = widget::column::with_capacity(refs.len()).spacing(space_xxs);

                for ref_name in refs {
                    let installed_version = package
                        .extra
                        .get(&format!("{}_installed", ref_name))
                        .map(|s| s.as_str());
                    let update_version = package
                        .extra
                        .get(&format!("{}_update", ref_name))
                        .map(|s| s.as_str());

                    let version_text = match (installed_version, update_version) {
                        (Some(installed), Some(update)) => {
                            format!("{}: {} → {}", ref_name, installed, update)
                        }
                        (Some(installed), None) => {
                            format!("{}: {}", ref_name, installed)
                        }
                        (None, Some(update)) => {
                            format!("{}: → {}", ref_name, update)
                        }
                        (None, None) => ref_name.to_string(),
                    };

                    package_list = package_list.push(widget::text(version_text));
                }

                return widget::column::with_capacity(2)
                    .push(widget::text::title4(fl!("system-package-updates")))
                    .push(widget::scrollable(package_list))
                    .width(Length::Fill)
                    .spacing(space_s)
                    .into();
            }
        }

        // Regular package release notes
        // Note: appstream releases are ordered from newest to oldest, so first() is the latest
        let (version, date, summary, url) = {
            self.updates
                .as_deref()
                .and_then(|updates| updates.get(index).map(|(_, package)| package))
                .and_then(|selected| {
                    selected.info.releases.first().map(|latest| {
                        (
                            &*latest.version,
                            latest.timestamp,
                            latest.description.to_owned(),
                            latest.url.as_deref(),
                        )
                    })
                })
                .unwrap_or(("", None, None, None))
        };
        widget::column::with_capacity(3)
            .push(
                widget::column::with_capacity(2)
                    .push(widget::text::title4(format!(
                        "{} {}",
                        fl!("latest-version"),
                        version
                    )))
                    .push_maybe(
                        date.and_then(|secs| {
                            chrono::DateTime::from_timestamp(secs, 0).map(|dt| {
                                dt.with_timezone(&chrono::Local)
                                    .format("%Y-%m-%d")
                                    .to_string()
                            })
                        })
                        .map(widget::text),
                    ),
            )
            .push(widget::scrollable(widget::text(
                summary.unwrap_or_else(|| fl!("no-description")),
            )))
            .push_maybe(url.map(widget::text))
            .width(Length::Fill)
            .spacing(space_s)
            .into()
    }

    pub fn sources(&self) -> Vec<Source> {
        let mut sources = Vec::new();
        if self.backends.contains_key(&BackendName::FlatpakUser) {
            sources.push(Source {
                backend_name: BackendName::FlatpakUser,
                id: "flathub".to_string(),
                name: "Flathub".to_string(),
                kind: SourceKind::Recommended {
                    data: include_bytes!("../res/flathub.flatpakrepo"),
                    enabled: false,
                },
                requires: Vec::new(),
            });
            sources.push(Source {
                backend_name: BackendName::FlatpakUser,
                id: "cosmic".to_string(),
                name: "COSMIC Flatpak".to_string(),
                kind: SourceKind::Recommended {
                    data: include_bytes!("../res/cosmic.flatpakrepo"),
                    enabled: false,
                },
                //TODO: can this be defined in flatpakrepo file?
                requires: vec!["flathub".to_string()],
            });
        }

        //TODO: check source URL?
        for (backend_name, backend) in self.backends.iter() {
            for cache in backend.info_caches() {
                let mut found_source = false;
                for source in sources.iter_mut() {
                    if *backend_name == source.backend_name && cache.source_id == source.id {
                        match &mut source.kind {
                            SourceKind::Recommended { enabled, .. } => {
                                *enabled = true;
                            }
                            SourceKind::Custom => {}
                        }
                        found_source = true;
                    }
                }
                //TODO: allow other backends to show sources?
                if !found_source && *backend_name == BackendName::FlatpakUser {
                    sources.push(Source {
                        backend_name: *backend_name,
                        id: cache.source_id.clone(),
                        name: cache.source_name.clone(),
                        kind: SourceKind::Custom,
                        requires: Vec::new(),
                    })
                }
            }
        }

        sources
    }

    fn repositories(&self) -> Element<'_, Message> {
        if !cfg!(feature = "flatpak") {
            return widget::text(fl!("no-flatpak")).into();
        }

        let sources = self.sources();
        let mut recommended = widget::settings::section().title(fl!("recommended-flatpak-sources"));
        let mut custom = widget::settings::section().header(widget::column::with_children(vec![
            widget::text::heading(fl!("custom-flatpak-sources")).into(),
            widget::text::body(fl!("import-flatpakrepo")).into(),
        ]));

        let mut has_custom_sources = false;

        for source in sources.iter() {
            let mut adds = Vec::new();
            let mut rms = Vec::new();
            if let Some(add) = source.add() {
                adds.push(add);
            }
            if let Some(rm) = source.remove() {
                rms.push(rm);
            }
            for other in sources.iter() {
                if source.backend_name == other.backend_name {
                    // Add other sources required by this source
                    if source.requires.contains(&other.id) {
                        if let Some(add) = other.add() {
                            adds.push(add);
                        }
                    }

                    // Remove other sources that require this source
                    if other.requires.contains(&source.id) {
                        if let Some(rm) = other.remove() {
                            rms.push(rm);
                        }
                    }
                }
            }

            let item =
                widget::settings::item::builder(source.name.clone()).description(source.id.clone());
            let element = match self
                .repos_changing
                .iter()
                .find(|x| x.0 == source.backend_name && x.1 == source.id)
                .map(|x| x.2)
            {
                Some(adding) => item.control(widget::text(if adding {
                    fl!("adding")
                } else {
                    fl!("removing")
                })),
                None => {
                    if !adds.is_empty() {
                        item.control(widget::button::text(fl!("add")).on_press_maybe(
                            if self.repos_changing.is_empty() {
                                Some(Message::RepositoryAdd(source.backend_name, adds.clone()))
                            } else {
                                None
                            },
                        ))
                    } else if !rms.is_empty() {
                        item.control(widget::button::text(fl!("remove")).on_press_maybe(
                            if self.repos_changing.is_empty() {
                                Some(Message::RepositoryRemove(source.backend_name, rms.clone()))
                            } else {
                                None
                            },
                        ))
                    } else {
                        item.control(widget::horizontal_space())
                    }
                }
            };

            match source.kind {
                SourceKind::Recommended { .. } => {
                    recommended = recommended.add(element);
                }
                SourceKind::Custom => {
                    has_custom_sources = true;
                    custom = custom.add(element);
                }
            }
        }
        // Add list item when no custom sources exist
        if !has_custom_sources {
            custom = custom.add(widget::text::body(fl!("no-custom-flatpak-sources")));
        }

        let custom = widget::column::with_children(vec![
            custom.into(),
            widget::container(widget::button::standard(fl!("import")).on_press_maybe(
                if self.repos_changing.is_empty() {
                    Some(Message::RepositoryAddDialog(BackendName::FlatpakUser))
                } else {
                    None
                },
            ))
            .width(Length::Fill)
            .align_x(Alignment::End)
            .into(),
        ])
        .spacing(theme::spacing().space_xxs);

        widget::settings::view_column(vec![recommended.into(), custom.into()]).into()
    }
}

/// Implement [`Application`] to integrate with COSMIC.
impl Application for App {
    /// Multithreaded async executor to use with the app.
    type Executor = executor::multi::Executor;

    /// Argument received
    type Flags = Flags;

    /// Message type specific to our [`App`].
    type Message = Message;

    /// The unique application ID to supply to the window manager.
    const APP_ID: &'static str = "com.system76.CosmicStore";

    fn core(&self) -> &Core {
        &self.core
    }

    fn core_mut(&mut self) -> &mut Core {
        &mut self.core
    }

    /// Creates the application, and optionally emits command on initialize.
    fn init(core: Core, flags: Self::Flags) -> (Self, Task<Self::Message>) {
        let locale = sys_locale::get_locale().unwrap_or_else(|| {
            log::warn!("failed to get system locale, falling back to en-US");
            String::from("en-US")
        });

        let app_themes = vec![fl!("match-desktop"), fl!("dark"), fl!("light")];

        let mut nav_model = widget::nav_bar::Model::default();
        for &nav_page in NavPage::all() {
            let id = nav_model
                .insert()
                .icon(nav_page.icon())
                .text(nav_page.title())
                .data::<NavPage>(nav_page)
                .id();
            if nav_page == NavPage::default() {
                //TODO: save last page?
                nav_model.activate(id);
            }
        }

        // Build buttons for applet placement dialog

        let mut applet_placement_buttons =
            cosmic::widget::segmented_button::SingleSelectModel::builder().build();
        let _ = applet_placement_buttons.insert().text(fl!("panel")).id();
        let _ = applet_placement_buttons.insert().text(fl!("dock")).id();
        applet_placement_buttons.activate_position(0);

        let mut app = App {
            core,
            config_handler: flags.config_handler,
            config: flags.config,
            mode: flags.mode,
            locale,
            app_themes,
            apps: Arc::new(Apps::new()),
            category_index: Arc::new(CategoryIndex::new()),
            backends: Backends::new(),
            context_page: ContextPage::Settings,
            dialog_pages: VecDeque::new(),
            explore_page_opt: None,
            key_binds: key_binds(),
            nav_model,
            #[cfg(feature = "notify")]
            notification_opt: None,
            pending_operation_id: 0,
            pending_operations: BTreeMap::new(),
            progress_operations: BTreeSet::new(),
            complete_operations: BTreeMap::new(),
            failed_operations: BTreeMap::new(),
            repos_changing: Vec::new(),
            scrollable_id: widget::Id::unique(),
            scroll_views: HashMap::new(),
            search_active: false,
            search_id: widget::Id::unique(),
            search_input: String::new(),
            size: Cell::new(None),
            installed: None,
            updates: None,
            waiting_installed: Vec::new(),
            waiting_updates: Vec::new(),
            category_results: None,
            category_load_start: None,
            explore_results: HashMap::new(),
            explore_load_start: None,
            installed_results: None,
            search_results: None,
            selected_opt: None,
            applet_placement_buttons,
            uninstall_purge_data: false,
        };

        // Load cached explore results for instant display
        let cache_start = Instant::now();
        if let Some(cached) = CachedExploreResults::load() {
            app.explore_results = cached.to_results();
            log::info!(
                "explore page loaded from cache: {} categories in {:?}",
                app.explore_results.len(),
                cache_start.elapsed()
            );
        }

        if let Some(subcommand) = flags.subcommand_opt {
            // Search for term
            app.search_active = true;
            app.search_input = subcommand;
        }

        match app.mode {
            Mode::Normal => {}
            Mode::GStreamer { .. } => {
                app.core.window.use_template = false;
            }
        }

        let command = Task::batch([app.update_title(), app.update_backends(true)]);
        (app, command)
    }

    fn nav_model(&self) -> Option<&widget::nav_bar::Model> {
        match self.mode {
            Mode::GStreamer { .. } => None,
            _ => Some(&self.nav_model),
        }
    }

    #[cfg(feature = "single-instance")]
    fn dbus_activation(&mut self, msg: cosmic::dbus_activation::Message) -> Task<Message> {
        let mut tasks = Vec::with_capacity(2);
        if self.core.main_window_id().is_none() {
            // Create window if required
            let (window_id, task) = window::open(window::Settings {
                min_size: Some(Size::new(420.0, 300.0)),
                decorations: false,
                exit_on_close_request: false,
                ..Default::default()
            });
            self.core.set_main_window_id(Some(window_id));
            tasks.push(task.map(|_id| action::none()));
        }
        if let cosmic::dbus_activation::Details::ActivateAction { action, .. } = msg.msg {
            // Search for term
            self.search_active = true;
            self.search_input = action;
            tasks.push(self.search());
        }
        Task::batch(tasks)
    }

    fn on_app_exit(&mut self) -> Option<Message> {
        Some(Message::WindowClose)
    }

    fn on_escape(&mut self) -> Task<Message> {
        if self.core.window.show_context {
            // Close context drawer if open
            self.core.window.show_context = false;
        } else if self.search_active {
            // Close search if open
            self.search_active = false;
            if self.search_results.take().is_some() {
                return self.update_scroll();
            }
        }
        Task::none()
    }

    fn on_nav_select(&mut self, id: widget::nav_bar::Id) -> Task<Message> {
        // Note: Don't clear category_results here to avoid flicker - new results will replace
        self.explore_page_opt = None;
        self.search_active = false;
        self.search_results = None;
        self.selected_opt = None;
        self.nav_model.activate(id);
        let mut commands = Vec::with_capacity(2);
        self.scroll_views.clear();
        commands.push(self.update_scroll());
        if let Some(categories) = self
            .nav_model
            .active_data::<NavPage>()
            .and_then(|nav_page| nav_page.categories())
        {
            // Start timing category page load
            self.category_load_start = Some(Instant::now());
            commands.push(self.categories(categories));
        }
        if let Some(NavPage::Updates) = self.nav_model.active_data::<NavPage>() {
            if self.updates.is_some() {
                commands.push(self.update_updates());
            }
        }
        Task::batch(commands)
    }

    /// Handle application events here.
    fn update(&mut self, message: Self::Message) -> Task<Message> {
        self.handle_update(message)
    }

    fn context_drawer(&self) -> Option<context_drawer::ContextDrawer<'_, Message>> {
        if !self.core.window.show_context {
            return None;
        }

        Some(match &self.context_page {
            ContextPage::Operations => context_drawer::context_drawer(
                self.operations(),
                Message::ToggleContextPage(ContextPage::Operations),
            )
            .title(fl!("operations")),
            ContextPage::Settings => context_drawer::context_drawer(
                self.settings(),
                Message::ToggleContextPage(ContextPage::Settings),
            )
            .title(fl!("settings")),
            ContextPage::ReleaseNotes(i, app_name) => context_drawer::context_drawer(
                self.release_notes(*i),
                Message::ToggleContextPage(ContextPage::ReleaseNotes(*i, app_name.clone())),
            )
            .title(app_name),
            ContextPage::Repositories => context_drawer::context_drawer(
                self.repositories(),
                Message::ToggleContextPage(ContextPage::Repositories),
            )
            .title(fl!("software-repositories")),
        })
    }

    fn dialog(&self) -> Option<Element<'_, Message>> {
        let dialog_page = self.dialog_pages.front()?;

        let dialog = match dialog_page {
            DialogPage::FailedOperation(id) => {
                //TODO: try next dialog page (making sure index is used by Dialog messages)?
                let (operation, _, err) = self.failed_operations.get(id)?;

                let (title, body) = operation.failed_dialog(err);
                widget::dialog()
                    .title(title)
                    .body(body)
                    .icon(widget::icon::from_name("dialog-error").size(64))
                    //TODO: retry action
                    .primary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
            }
            DialogPage::RepositoryAddError(err) => {
                widget::dialog()
                    .title(fl!("repository-add-error-title"))
                    .body(err)
                    .icon(widget::icon::from_name("dialog-error").size(64))
                    //TODO: retry action
                    .primary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
            }
            DialogPage::RepositoryRemove(_backend_name, repo_rm) => {
                let mut list = widget::list::list_column();
                //TODO: fix max dialog height in libcosmic?
                let mut scrollable_height = 0.0;
                for (i, (_id, name)) in repo_rm.installed.iter().enumerate() {
                    if i > 0 {
                        //TODO: add correct padding per item
                        scrollable_height += 0.0;
                    }
                    //TODO: show icons
                    list = list.add(widget::text(name));
                    scrollable_height += 32.0;
                }
                widget::dialog()
                    .title(fl!(
                        "repository-remove-title",
                        name = repo_rm.rms[0].name.as_str()
                    ))
                    .body(fl!(
                        "repository-remove-body",
                        dependency = repo_rm.rms.get(1).map_or("none", |rm| rm.name.as_str())
                    ))
                    .control(
                        widget::scrollable(list).height(if let Some(size) = self.size.get() {
                            let max_size = (size.height - 192.0).min(480.0);
                            if scrollable_height > max_size {
                                Length::Fixed(max_size)
                            } else {
                                Length::Shrink
                            }
                        } else {
                            Length::Fill
                        }),
                    )
                    .primary_action(
                        widget::button::destructive(fl!("remove")).on_press(Message::DialogConfirm),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
            }
            DialogPage::Uninstall(backend_name, _id, info) => {
                let is_flatpak = backend_name.is_flatpak();
                let mut dialog = widget::dialog()
                    .title(fl!("uninstall-app", name = info.name.as_str()))
                    .body(if is_flatpak {
                        fl!("uninstall-app-flatpak-warning", name = info.name.as_str())
                    } else {
                        fl!("uninstall-app-warning", name = info.name.as_str())
                    })
                    .icon(widget::icon::from_name(Self::APP_ID).size(64));

                // Only show data deletion option for Flatpak apps
                if is_flatpak {
                    dialog = dialog.control(
                        widget::checkbox(fl!("delete-app-data"), self.uninstall_purge_data)
                            .on_toggle(Message::ToggleUninstallPurgeData),
                    );
                }

                dialog
                    .primary_action(
                        widget::button::destructive(fl!("uninstall"))
                            .on_press(Message::DialogConfirm),
                    )
                    .secondary_action(
                        widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                    )
            }
            DialogPage::Place(id) => widget::dialog()
                .title(fl!("place-applet"))
                .body(fl!("place-applet-desc"))
                .control(
                    widget::row().push(
                        cosmic::widget::segmented_control::horizontal(
                            &self.applet_placement_buttons,
                        )
                        .on_activate(Message::SelectPlacement)
                        .minimum_button_width(0),
                    ),
                )
                .primary_action(
                    widget::button::suggested(fl!("place-and-refine"))
                        .on_press(Message::PlaceApplet(id.clone())),
                )
                .secondary_action(
                    widget::button::standard(fl!("cancel")).on_press(Message::DialogCancel),
                ),
        };

        Some(dialog.into())
    }

    fn footer(&self) -> Option<Element<'_, Message>> {
        if self.progress_operations.is_empty() {
            return None;
        }

        let cosmic_theme::Spacing {
            space_xxs,
            space_xs,
            space_s,
            ..
        } = theme::active().cosmic().spacing;

        let mut title = String::new();
        let mut total_progress = 0.0;
        let mut count = 0;
        for (_id, (op, progress)) in self.pending_operations.iter() {
            if title.is_empty() {
                title = op.pending_text(*progress as i32);
            }
            total_progress += progress;
            count += 1;
        }
        let running = count;
        // Adjust the progress bar so it does not jump around when operations finish
        for id in self.progress_operations.iter() {
            if self.complete_operations.contains_key(id) {
                total_progress += 100.0;
                count += 1;
            }
        }
        let finished = count - running;
        total_progress /= count as f32;
        if running > 1 {
            if finished > 0 {
                title = fl!(
                    "operations-running-finished",
                    running = running,
                    finished = finished,
                    percent = (total_progress as i32)
                );
            } else {
                title = fl!(
                    "operations-running",
                    running = running,
                    percent = (total_progress as i32)
                );
            }
        }

        //TODO: get height from theme?
        let progress_bar_height = Length::Fixed(4.0);
        let progress_bar =
            widget::progress_bar(0.0..=100.0, total_progress).height(progress_bar_height);

        let container = widget::layer_container(widget::column::with_children(vec![
            progress_bar.into(),
            widget::Space::with_height(space_xs).into(),
            widget::text::body(title).into(),
            widget::Space::with_height(space_s).into(),
            widget::row::with_children(vec![
                widget::button::link(fl!("details"))
                    .on_press(Message::ToggleContextPage(ContextPage::Operations))
                    .padding(0)
                    .trailing_icon(true)
                    .into(),
                widget::horizontal_space().into(),
                widget::button::standard(fl!("dismiss"))
                    .on_press(Message::PendingDismiss)
                    .into(),
            ])
            .align_y(Alignment::Center)
            .into(),
        ]))
        .padding([space_xxs, space_xs])
        .layer(cosmic_theme::Layer::Primary);

        Some(container.into())
    }

    fn header_start(&self) -> Vec<Element<'_, Message>> {
        match self.mode {
            Mode::Normal => vec![if self.search_active {
                widget::text_input::search_input("", &self.search_input)
                    .width(Length::Fixed(240.0))
                    .id(self.search_id.clone())
                    .on_clear(Message::SearchClear)
                    .on_input(Message::SearchInput)
                    .on_submit(Message::SearchSubmit)
                    .into()
            } else {
                widget::button::icon(widget::icon::from_name("system-search-symbolic"))
                    .on_press(Message::SearchActivate)
                    .padding(8)
                    .into()
            }],
            Mode::GStreamer { .. } => Vec::new(),
        }
    }

    fn header_end(&self) -> Vec<Element<'_, Message>> {
        match self.mode {
            Mode::Normal => {
                vec![
                    widget::tooltip(
                        widget::button::icon(widget::icon::from_name("application-menu-symbolic"))
                            .on_press(Message::ToggleContextPage(ContextPage::Repositories)),
                        widget::text(fl!("manage-repositories")),
                        widget::tooltip::Position::Bottom,
                    )
                    .into(),
                ]
            }
            Mode::GStreamer { .. } => Vec::new(),
        }
    }

    /// Creates a view after each update.
    fn view(&self) -> Element<'_, Self::Message> {
        let cosmic_theme::Spacing {
            space_s,
            space_xs,
            space_xxs,
            ..
        } = theme::active().cosmic().spacing;

        let content: Element<_> = match &self.mode {
            Mode::Normal => widget::responsive(move |mut size| {
                size.width = size.width.min(MAX_GRID_WIDTH);
                widget::scrollable(
                    widget::container(
                        widget::container(self.view_responsive(size)).max_width(MAX_GRID_WIDTH),
                    )
                    .align_x(Alignment::Center),
                )
                .id(self.scrollable_id.clone())
                .on_scroll(Message::ScrollView)
                .into()
            })
            .into(),
            Mode::GStreamer {
                codec,
                selected,
                installing,
            } => {
                //TODO: share code with DialogPage?
                let mut dialog = widget::dialog()
                    .icon(widget::icon::from_name("dialog-question").size(64))
                    .title(fl!("codec-title"))
                    .body(fl!(
                        "codec-header",
                        application = codec.application.as_str(),
                        description = codec.description.as_str()
                    ));
                if *installing {
                    let mut list = widget::list_column();

                    for (_id, (op, progress)) in self.pending_operations.iter().rev() {
                        list = list.add(widget::column::with_children(vec![
                            widget::progress_bar(0.0..=100.0, *progress)
                                .height(Length::Fixed(4.0))
                                .into(),
                            widget::Space::with_height(space_xs).into(),
                            widget::text(op.pending_text(*progress as i32)).into(),
                        ]));
                    }

                    for (_id, (op, progress, error)) in self.failed_operations.iter().rev() {
                        list = list.add(widget::column::with_children(vec![
                            widget::text(op.pending_text(*progress as i32)).into(),
                            widget::text(error).into(),
                        ]));
                    }

                    for (_id, op) in self.complete_operations.iter().rev() {
                        list = list.add(widget::text(op.completed_text()));
                    }

                    dialog = dialog.control(widget::scrollable(list));
                    if self.pending_operations.is_empty() {
                        let code = if self.failed_operations.is_empty() {
                            dialog = dialog.control(widget::text(fl!("codec-installed")));
                            GStreamerExitCode::Success
                        } else {
                            dialog = dialog.control(widget::text(fl!("codec-error")));
                            GStreamerExitCode::Error
                        };
                        dialog = dialog.secondary_action(
                            widget::button::standard(fl!("close"))
                                .on_press(Message::GStreamerExit(code)),
                        );
                    }
                } else {
                    match &self.search_results {
                        Some((_input, results)) => {
                            let mut list = widget::list_column();
                            for (i, result) in results.iter().enumerate() {
                                list = list.add(
                                    widget::mouse_area(
                                        widget::button::custom(
                                            widget::row::with_children(vec![
                                                widget::column::with_children(vec![
                                                    widget::text::body(&result.info.name).into(),
                                                    widget::text::caption(&result.info.summary)
                                                        .into(),
                                                ])
                                                .into(),
                                                widget::horizontal_space().into(),
                                                if selected.contains(&i) {
                                                    widget::icon::from_name(
                                                        "checkbox-checked-symbolic",
                                                    )
                                                    .size(16)
                                                    .into()
                                                } else {
                                                    widget::Space::with_width(Length::Fixed(16.0))
                                                        .into()
                                                },
                                            ])
                                            .spacing(space_s)
                                            .align_y(Alignment::Center),
                                        )
                                        .width(Length::Fill)
                                        .class(theme::Button::MenuItem)
                                        .force_enabled(true),
                                    )
                                    .on_press(Message::GStreamerToggle(i)),
                                );
                            }
                            dialog = dialog.control(widget::scrollable(list)).control(
                                widget::row::with_children(vec![
                                    widget::icon::from_name("dialog-warning").size(16).into(),
                                    widget::text(fl!("codec-footer")).into(),
                                ])
                                .spacing(space_xxs),
                            );
                        }
                        None => {
                            //TODO: loading indicator?
                            //column = column.push(widget::text("Loading..."));
                        }
                    }
                    let mut install_button = widget::button::suggested(fl!("install"));
                    if !selected.is_empty() {
                        install_button = install_button.on_press(Message::GStreamerInstall);
                    }
                    dialog = dialog.primary_action(install_button).secondary_action(
                        widget::button::standard(fl!("cancel"))
                            .on_press(Message::GStreamerExit(GStreamerExitCode::UserAbort)),
                    )
                }
                dialog
                    .control(widget::vertical_space())
                    .width(Length::Fill)
                    .height(Length::Fill)
                    .into()
            }
        };

        // Uncomment to debug layout:
        //content.explain(cosmic::iced::Color::WHITE)
        content
    }

    fn view_window(&self, _id: window::Id) -> Element<'_, Message> {
        // When closing the main window, view_window may be called after the main window is unset
        widget::horizontal_space().into()
    }

    fn subscription(&self) -> Subscription<Self::Message> {
        struct ConfigSubscription;
        struct ThemeSubscription;

        let mut subscriptions = vec![
            event::listen_with(|event, status, _window_id| match event {
                Event::Keyboard(KeyEvent::KeyPressed {
                    key,
                    modifiers,
                    text,
                    ..
                }) => match status {
                    event::Status::Ignored => Some(Message::Key(modifiers, key, text)),
                    event::Status::Captured => None,
                },
                Event::Window(WindowEvent::CloseRequested) => Some(Message::WindowClose),
                _ => None,
            }),
            cosmic_config::config_subscription(
                TypeId::of::<ConfigSubscription>(),
                Self::APP_ID.into(),
                CONFIG_VERSION,
            )
            .map(|update| {
                if !update.errors.is_empty() {
                    log::debug!("errors loading config: {:?}", update.errors);
                }
                Message::SystemThemeModeChange(update.config)
            }),
            cosmic_config::config_subscription::<_, cosmic_theme::ThemeMode>(
                TypeId::of::<ThemeSubscription>(),
                cosmic_theme::THEME_MODE_ID.into(),
                cosmic_theme::ThemeMode::version(),
            )
            .map(|update| {
                if !update.errors.is_empty() {
                    log::debug!("errors loading theme mode: {:?}", update.errors);
                }
                Message::SystemThemeModeChange(update.config)
            }),
        ];

        if self.config.update_check_interval_minutes > 0 {
            let duration =
                std::time::Duration::from_secs(self.config.update_check_interval_minutes * 60);
            subscriptions
                .push(cosmic::iced::time::every(duration).map(|_| Message::PeriodicUpdateCheck));
        }

        if !self.pending_operations.is_empty() {
            #[cfg(feature = "logind")]
            {
                struct InhibitSubscription;
                subscriptions.push(Subscription::run_with_id(
                    TypeId::of::<InhibitSubscription>(),
                    stream::channel(1, move |_msg_tx| async move {
                        let _inhibits = logind::inhibit().await;
                        pending().await
                    }),
                ));
            }

            #[cfg(feature = "notify")]
            if self.core.main_window_id().is_none() {
                struct NotificationSubscription;
                subscriptions.push(Subscription::run_with_id(
                    TypeId::of::<NotificationSubscription>(),
                    stream::channel(1, move |msg_tx| async move {
                        let msg_tx = Arc::new(tokio::sync::Mutex::new(msg_tx));
                        tokio::task::spawn_blocking(move || match notify_rust::Notification::new()
                            .summary(&fl!("notification-in-progress"))
                            .auto_icon()
                            .show()
                        {
                            Ok(notification) => {
                                let _ = futures::executor::block_on(async {
                                    msg_tx
                                        .lock()
                                        .await
                                        .send(Message::Notification(Arc::new(Mutex::new(
                                            notification,
                                        ))))
                                        .await
                                });
                            }
                            Err(err) => {
                                log::warn!("failed to create notification: {}", err);
                            }
                        })
                        .await
                        .unwrap();

                        pending().await
                    }),
                ));
            }
        }

        for (id, (op, _)) in self.pending_operations.iter() {
            //TODO: use recipe?
            let id = *id;
            let backend_opt = self.backends.get(&op.backend_name).cloned();
            let op = op.clone();
            subscriptions.push(Subscription::run_with_id(
                id,
                stream::channel(16, move |msg_tx| async move {
                    let msg_tx = Arc::new(tokio::sync::Mutex::new(msg_tx));
                    let res = match backend_opt {
                        Some(backend) => {
                            let on_progress = {
                                let msg_tx = msg_tx.clone();
                                Box::new(move |progress| {
                                    let _ = futures::executor::block_on(async {
                                        msg_tx
                                            .lock()
                                            .await
                                            .send(Message::PendingProgress(id, progress))
                                            .await
                                    });
                                })
                            };
                            let msg_tx = msg_tx.clone();
                            tokio::task::spawn_blocking(move || {
                                match backend.operation(&op, on_progress) {
                                    Ok(()) => Ok(()),
                                    Err(err) => match err.downcast_ref::<RepositoryRemoveError>() {
                                        Some(repo_rm) => {
                                            let _ = futures::executor::block_on(async {
                                                msg_tx
                                                    .lock()
                                                    .await
                                                    .send(Message::DialogPage(
                                                        DialogPage::RepositoryRemove(
                                                            op.backend_name,
                                                            repo_rm.clone(),
                                                        ),
                                                    ))
                                                    .await
                                            });
                                            Ok(())
                                        }
                                        None => Err(err.to_string()),
                                    },
                                }
                            })
                            .await
                            .unwrap()
                        }
                        None => Err(format!("backend {:?} not found", op.backend_name)),
                    };

                    match res {
                        Ok(()) => {
                            let _ = msg_tx.lock().await.send(Message::PendingComplete(id)).await;
                        }
                        Err(err) => {
                            let _ = msg_tx
                                .lock()
                                .await
                                .send(Message::PendingError(id, err))
                                .await;
                        }
                    }
                    pending().await
                }),
            ));
        }

        if let Some(selected) = &self.selected_opt {
            for (screenshot_i, screenshot) in selected.info.screenshots.iter().enumerate() {
                let url = screenshot.url.clone();
                subscriptions.push(Subscription::run_with_id(
                    url.clone(),
                    stream::channel(16, move |mut msg_tx| async move {
                        log::info!("fetch screenshot {}", url);
                        match reqwest::get(&url).await {
                            Ok(response) => match response.bytes().await {
                                Ok(bytes) => {
                                    log::info!(
                                        "fetched screenshot from {}: {} bytes",
                                        url,
                                        bytes.len()
                                    );
                                    let _ = msg_tx
                                        .send(Message::SelectedScreenshot(
                                            screenshot_i,
                                            url,
                                            bytes.to_vec(),
                                        ))
                                        .await;
                                }
                                Err(err) => {
                                    log::warn!("failed to read screenshot from {}: {}", url, err);
                                }
                            },
                            Err(err) => {
                                log::warn!("failed to request screenshot from {}: {}", url, err);
                            }
                        }
                        pending().await
                    }),
                ));
            }
        }

        Subscription::batch(subscriptions)
    }
}
