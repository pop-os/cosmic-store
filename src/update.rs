// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::collections::HashMap;
use std::env;
use std::process;
use std::time::Instant;

use cosmic::{
    action,
    app::Task,
    iced::{
        Size,
        keyboard::{Key, key},
        window::{self},
    },
    widget,
};

#[cfg(feature = "wayland")]
use cosmic::cosmic_config::CosmicConfigEntry;
#[cfg(feature = "wayland")]
use cosmic_panel_config::CosmicPanelConfig;

use crate::backend::BackendName;
use crate::explore::ExplorePage;
use crate::nav::NavPage;
use crate::operation::{Operation, OperationKind, RepositoryAdd};
use crate::search::{apply_icons_to_results, preserve_icons_from};
use crate::{App, DialogPage, GStreamerExitCode, Message, Mode};

impl App {
    pub fn handle_update(&mut self, message: Message) -> Task<Message> {
        // Helper for updating config values efficiently
        macro_rules! config_set {
            ($name: ident, $value: expr) => {
                match &self.config_handler {
                    Some(config_handler) => {
                        match paste::paste! { self.config.[<set_ $name>](config_handler, $value) } {
                            Ok(_) => {}
                            Err(err) => {
                                log::warn!(
                                    "failed to save config {:?}: {}",
                                    stringify!($name),
                                    err
                                );
                            }
                        }
                    }
                    None => {
                        self.config.$name = $value;
                        log::warn!(
                            "failed to save config {:?}: no config handler",
                            stringify!($name)
                        );
                    }
                }
            };
        }

        match message {
            Message::AppTheme(app_theme) => {
                config_set!(app_theme, app_theme);
                return self.update_config();
            }
            Message::Backends(backends) => {
                self.backends = backends;
                self.repos_changing.clear();
                // Note: Don't clear explore_results to avoid flicker - fresh results will overwrite
                let mut tasks = Vec::with_capacity(2);
                tasks.push(self.update_installed());
                match self.mode {
                    Mode::Normal => {
                        tasks.push(self.update_updates());
                    }
                    Mode::GStreamer { .. } => {}
                }
                return Task::batch(tasks);
            }
            Message::CategoryResults(categories, mut results) => {
                if let Some(start) = self.category_load_start.take() {
                    log::info!(
                        "category page loaded: {} results in {:?}",
                        results.len(),
                        start.elapsed()
                    );
                }
                if let Some((_, old_results)) = &self.category_results {
                    preserve_icons_from(old_results, &mut results);
                }
                self.category_results = Some((categories, results));
                // Load icons in background
                return Task::batch([self.update_scroll(), self.load_category_icons(categories)]);
            }
            Message::CategoryIconsLoaded(categories, icons) => {
                if let Some((cats, results)) = &mut self.category_results {
                    if *cats == categories {
                        apply_icons_to_results(results, icons);
                    }
                }
            }
            Message::CheckUpdates | Message::PeriodicUpdateCheck => {
                if matches!(message, Message::PeriodicUpdateCheck) {
                    log::info!("periodic background update check triggered");
                }
                //TODO: this only checks updates if they have already been checked
                if self.updates.take().is_some() {
                    if self.pending_operations.is_empty() {
                        return self.update_backends(true);
                    } else {
                        log::warn!("cannot check for updates, operations are in progress");
                    }
                } else {
                    log::warn!("already checking for updates");
                }
            }
            Message::Config(config) => {
                if config != self.config {
                    log::info!("update config");
                    //TODO: update syntax theme by clearing tabs, only if needed
                    self.config = config;
                    return self.update_config();
                }
            }
            Message::DialogCancel => {
                self.dialog_pages.pop_front();
                self.uninstall_purge_data = false;
            }
            Message::DialogConfirm => match self.dialog_pages.pop_front() {
                Some(DialogPage::RepositoryRemove(backend_name, repo_rm)) => {
                    self.operation(Operation {
                        kind: OperationKind::RepositoryRemove(repo_rm.rms, true),
                        backend_name,
                        package_ids: Vec::new(),
                        infos: Vec::new(),
                    });
                }
                Some(DialogPage::Uninstall(backend_name, id, info)) => {
                    let purge_data = self.uninstall_purge_data;
                    self.uninstall_purge_data = false;
                    return self.handle_update(Message::Operation(
                        OperationKind::Uninstall { purge_data },
                        backend_name,
                        id,
                        info,
                    ));
                }
                _ => {}
            },
            Message::DialogPage(dialog_page) => {
                self.dialog_pages.push_back(dialog_page);
            }
            Message::ExplorePage(explore_page_opt) => {
                self.explore_page_opt = explore_page_opt;
                return self.update_scroll();
            }
            Message::AllExploreResults(mut all_results, cached) => {
                let mut tasks = Vec::new();
                for &explore_page in ExplorePage::all() {
                    if let Some(mut results) = all_results.remove(&explore_page) {
                        if let Some(old_results) = self.explore_results.get(&explore_page) {
                            preserve_icons_from(old_results, &mut results);
                        }
                        self.explore_results.insert(explore_page, results);
                        tasks.push(self.load_explore_icons(explore_page));
                    }
                }

                if let Some(start) = self.explore_load_start.take() {
                    log::info!(
                        "explore page reloaded after data fetch: {} categories in {:?}",
                        self.explore_results.len(),
                        start.elapsed()
                    );
                }

                // Save pre-built cache in background
                tasks.push(Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || {
                            cached.save().map_err(|e| e.to_string())
                        })
                        .await
                        .unwrap_or_else(|e| Err(e.to_string()))
                    },
                    |result| action::app(Message::ExploreCacheSaved(result)),
                ));

                return Task::batch(tasks);
            }
            Message::ExploreCacheSaved(result) => match result {
                Ok(()) => log::info!("explore cache saved"),
                Err(err) => log::warn!("failed to save explore cache: {}", err),
            },
            Message::ExploreIconsLoaded(explore_page, icons) => {
                if let Some(results) = self.explore_results.get_mut(&explore_page) {
                    apply_icons_to_results(results, icons);
                }
            }
            Message::GStreamerExit(code) => match self.mode {
                Mode::Normal => {}
                Mode::GStreamer { .. } => {
                    process::exit(code as i32);
                }
            },
            Message::GStreamerInstall => {
                let mut ops = Vec::new();
                match &mut self.mode {
                    Mode::Normal => {}
                    Mode::GStreamer {
                        selected,
                        installing,
                        ..
                    } => {
                        if let Some((_input, results)) = &self.search_results {
                            for (i, result) in results.iter().enumerate() {
                                let installed = Self::is_installed_inner(
                                    &self.installed,
                                    result.backend_name,
                                    &result.id,
                                    &result.info,
                                );
                                if installed != selected.contains(&i) {
                                    let kind = if installed {
                                        OperationKind::Uninstall { purge_data: false }
                                    } else {
                                        OperationKind::Install
                                    };
                                    eprintln!(
                                        "{:?} {:?} from backend {} and info {:?}",
                                        kind, result.id, result.backend_name, result.info
                                    );
                                    ops.push(Operation {
                                        kind,
                                        backend_name: result.backend_name,
                                        package_ids: vec![result.id.clone()],
                                        infos: vec![result.info.clone()],
                                    });
                                }
                            }
                            *installing = true;
                        }
                    }
                }
                for op in ops {
                    self.operation(op);
                }
            }
            Message::GStreamerToggle(i) => match &mut self.mode {
                Mode::Normal => {}
                Mode::GStreamer { selected, .. } => {
                    if !selected.remove(&i) {
                        selected.insert(i);
                    }
                }
            },
            Message::Installed(installed) => {
                self.installed = Some(installed);
                self.waiting_installed.clear();

                return self.update_apps();
            }
            Message::AppsUpdated(apps, category_index) => {
                self.apps = apps;
                self.category_index = category_index;

                // Update selected sources (lightweight, stays on main thread)
                {
                    let sources_opt = self.selected_opt.as_ref().map(|selected| {
                        self.selected_sources(selected.backend_name, &selected.id, &selected.info)
                    });
                    if let Some(sources) = sources_opt {
                        if let Some(selected) = &mut self.selected_opt {
                            selected.sources = sources;
                        }
                    }
                }

                log::info!("updated app cache with {} ids", self.apps.len());

                let mut commands = Vec::new();
                //TODO: search not done if item is selected because that would clear selection
                if self.search_active && self.selected_opt.is_none() {
                    // Update search if active
                    commands.push(self.search());
                }
                match self.mode {
                    Mode::Normal => {
                        if let Some(categories) = self
                            .nav_model
                            .active_data::<NavPage>()
                            .and_then(|nav_page| nav_page.categories())
                        {
                            self.category_load_start = Some(Instant::now());
                            commands.push(self.categories(categories));
                        }
                        commands.push(self.installed_results());
                        // Start timing explore page loading
                        self.explore_load_start = Some(Instant::now());
                        commands.push(self.explore_results_all());
                    }
                    Mode::GStreamer { .. } => {}
                }
                return Task::batch(commands);
            }
            Message::InstalledResults(installed_results) => {
                self.installed_results = Some(installed_results);
                // Load icons in background
                return self.load_installed_icons();
            }
            Message::InstalledIconsLoaded(icons) => {
                if let Some(results) = &mut self.installed_results {
                    apply_icons_to_results(results, icons);
                }
            }
            Message::Key(modifiers, key, text) => {
                // Handle ESC key to close dialogs
                if !self.dialog_pages.is_empty()
                    && matches!(key, Key::Named(key::Named::Escape))
                    && !modifiers.logo()
                    && !modifiers.control()
                    && !modifiers.alt()
                    && !modifiers.shift()
                {
                    return self.handle_update(Message::DialogCancel);
                }

                for (key_bind, action) in self.key_binds.iter() {
                    if key_bind.matches(modifiers, &key) {
                        return self.handle_update(action.message());
                    }
                }

                // Uncaptured keys with only shift modifiers go to the search box
                if !modifiers.logo()
                    && !modifiers.control()
                    && !modifiers.alt()
                    && matches!(key, Key::Character(_))
                {
                    if let Some(text) = text {
                        self.search_active = true;
                        self.search_input.push_str(&text);
                        return Task::batch([
                            widget::text_input::focus(self.search_id.clone()),
                            self.search(),
                        ]);
                    }
                }
            }
            Message::LaunchUrl(url) => match open::that_detached(&url) {
                Ok(()) => {}
                Err(err) => {
                    log::warn!("failed to open {:?}: {}", url, err);
                }
            },
            Message::MaybeExit => {
                if self.core.main_window_id().is_none() && self.pending_operations.is_empty() {
                    // Exit if window is closed and there are no pending operations
                    process::exit(0);
                }
            }
            #[cfg(feature = "notify")]
            Message::Notification(notification) => {
                self.notification_opt = Some(notification);
            }
            Message::OpenDesktopId(desktop_id) => {
                return self.open_desktop_id(desktop_id);
            }
            Message::Operation(kind, backend_name, package_id, info) => {
                self.operation(Operation {
                    kind,
                    backend_name,
                    package_ids: vec![package_id],
                    infos: vec![info],
                });
            }
            Message::PendingComplete(id) => {
                if let Some((op, _)) = self.pending_operations.remove(&id) {
                    for (package_id, info) in op.package_ids.iter().zip(op.infos.iter()) {
                        self.waiting_installed.push((
                            op.backend_name,
                            info.source_id.clone(),
                            package_id.clone(),
                        ));
                        self.waiting_updates.push((
                            op.backend_name,
                            info.source_id.clone(),
                            package_id.clone(),
                        ));
                    }
                    self.complete_operations.insert(id, op);
                }
                // Close progress notification if all relavent operations are finished
                if self.pending_operations.is_empty() {
                    self.progress_operations.clear();

                    // If repos were changing, update backends
                    if !self.repos_changing.is_empty() {
                        return Task::batch([
                            self.update_notification(),
                            self.update_backends(true),
                        ]);
                    }
                }
                return Task::batch([
                    self.update_notification(),
                    self.update_installed(),
                    self.update_updates(),
                ]);
            }
            Message::PendingDismiss => {
                self.progress_operations.clear();
            }
            Message::PendingError(id, err) => {
                log::warn!("operation {id} failed: {err}");
                if let Some((op, progress)) = self.pending_operations.remove(&id) {
                    self.failed_operations.insert(id, (op, progress, err));
                    self.dialog_pages.push_back(DialogPage::FailedOperation(id));
                }
                // Close progress notification if all relavent operations are finished
                if self.pending_operations.is_empty() {
                    self.progress_operations.clear();

                    // If repos were changing, update backends
                    if !self.repos_changing.is_empty() {
                        return Task::batch([
                            self.update_notification(),
                            self.update_backends(true),
                        ]);
                    }
                }
                return self.update_notification();
            }
            Message::PendingProgress(id, new_progress) => {
                if let Some((_, progress)) = self.pending_operations.get_mut(&id) {
                    *progress = new_progress;
                }
                return self.update_notification();
            }
            Message::RepositoryAdd(backend_name, adds) => {
                self.operation(Operation {
                    kind: OperationKind::RepositoryAdd(adds),
                    backend_name,
                    package_ids: Vec::new(),
                    infos: Vec::new(),
                });
            }
            Message::RepositoryAddDialog(backend_name) => {
                //TODO: support other backends?
                if backend_name == BackendName::FlatpakUser {
                    #[cfg(feature = "xdg-portal")]
                    return Task::perform(
                        async move {
                            use cosmic::dialog::file_chooser::{self, FileFilter};
                            let error_dialog = |err| {
                                action::app(Message::DialogPage(DialogPage::RepositoryAddError(
                                    err,
                                )))
                            };
                            let filter = FileFilter::new("Flatpak repo file").glob("*.flatpakrepo");
                            let dialog = file_chooser::open::Dialog::new().filter(filter);
                            let path = match dialog.open_file().await {
                                Ok(response) => {
                                    let url = response.url();
                                    match url.scheme() {
                                        "file" => url.to_file_path().unwrap(),
                                        other => {
                                            return error_dialog(format!(
                                                "{url} has unknown scheme: {other}"
                                            ));
                                        }
                                    }
                                }
                                Err(file_chooser::Error::Cancelled) => {
                                    return action::none();
                                }
                                Err(err) => {
                                    return error_dialog(format!(
                                        "failed to import repository: {err}"
                                    ));
                                }
                            };
                            let id = match path.file_stem() {
                                Some(stem) => stem.to_string_lossy().to_string(),
                                None => {
                                    return error_dialog(format!(
                                        "{path:?} does not have file stem"
                                    ));
                                }
                            };
                            let data = match tokio::fs::read(&path).await {
                                Ok(ok) => ok,
                                Err(err) => {
                                    return error_dialog(format!("failed to read {path:?}: {err}"));
                                }
                            };
                            action::app(Message::RepositoryAdd(
                                backend_name,
                                vec![RepositoryAdd { id, data }],
                            ))
                        },
                        |x| x,
                    );
                }
                log::error!("no support for adding repositories to {}", backend_name);
            }
            Message::RepositoryRemove(backend_name, rms) => {
                self.operation(Operation {
                    kind: OperationKind::RepositoryRemove(rms, false),
                    backend_name,
                    package_ids: Vec::new(),
                    infos: Vec::new(),
                });
            }
            Message::ScrollView(viewport) => {
                self.scroll_views.insert(self.scroll_context(), viewport);
            }
            Message::SearchActivate => {
                self.search_active = true;
                return widget::text_input::focus(self.search_id.clone());
            }
            Message::SearchClear => {
                self.search_active = false;
                self.search_input.clear();
                if self.search_results.take().is_some() {
                    return self.update_scroll();
                }
            }
            Message::SearchInput(input) => {
                if input != self.search_input {
                    self.search_input = input;
                    // This performs live search
                    if !self.search_input.is_empty() {
                        return self.search();
                    }
                }
            }
            Message::SearchResults(input, mut results, auto_select) => {
                if input == self.search_input {
                    if let Some((_, old_results)) = &self.search_results {
                        preserve_icons_from(old_results, &mut results);
                    }
                    // Clear selected item so search results can be shown
                    self.selected_opt = None;
                    if auto_select && results.len() == 1 {
                        // This drops update_scroll's command, it will be done again later
                        let _ = self.select(
                            results[0].backend_name,
                            results[0].id.clone(),
                            results[0].icon_opt.clone(),
                            results[0].info.clone(),
                        );
                    }
                    let mut tasks = Vec::with_capacity(2);
                    match &mut self.mode {
                        Mode::Normal => {}
                        Mode::GStreamer { selected, .. } => {
                            // Update selected results for gstreamer mode
                            selected.clear();
                            if results.is_empty() {
                                // No results, means we should exit
                                return self.handle_update(Message::GStreamerExit(
                                    GStreamerExitCode::NotFound,
                                ));
                            }
                            for (i, result) in results.iter().enumerate() {
                                if Self::is_installed_inner(
                                    &self.installed,
                                    result.backend_name,
                                    &result.id,
                                    &result.info,
                                ) {
                                    selected.insert(i);
                                }
                            }
                            // Create window if needed
                            if self.core.main_window_id().is_none() {
                                // Create window if required
                                let size = Size::new(640.0, 464.0);
                                let mut settings = window::Settings {
                                    decorations: false,
                                    exit_on_close_request: false,
                                    min_size: Some(size),
                                    resizable: true,
                                    size,
                                    transparent: true,
                                    ..Default::default()
                                };

                                #[cfg(target_os = "linux")]
                                {
                                    // Use the dialog ID to make it float
                                    settings.platform_specific.application_id =
                                        "com.system76.CosmicStoreDialog".to_string();
                                }

                                let (window_id, task) = window::open(settings);
                                self.core.set_main_window_id(Some(window_id));
                                tasks.push(task.map(|_id| action::none()));
                            }
                        }
                    }
                    self.search_results = Some((input.clone(), results));
                    tasks.push(self.update_scroll());
                    // Load icons in background
                    tasks.push(self.load_search_icons(input));
                    return Task::batch(tasks);
                } else {
                    log::warn!(
                        "received {} results for {:?} after search changed to {:?}",
                        results.len(),
                        input,
                        self.search_input
                    );
                }
            }
            Message::SearchIconsLoaded(input, icons) => {
                if let Some((query, results)) = &mut self.search_results {
                    if *query == input {
                        apply_icons_to_results(results, icons);
                    }
                }
            }
            Message::SearchSubmit(_search_input) => {
                if !self.search_input.is_empty() {
                    return self.search();
                }
            }
            Message::Select(backend_name, id, icon, info) => {
                return self.select(backend_name, id, icon, info);
            }
            Message::SelectInstalled(result_i) => {
                if let Some(results) = &self.installed_results {
                    match results.get(result_i) {
                        Some(result) => {
                            return self.select(
                                result.backend_name,
                                result.id.clone(),
                                result.icon_opt.clone(),
                                result.info.clone(),
                            );
                        }
                        None => {
                            log::error!("failed to find installed result with index {}", result_i);
                        }
                    }
                }
            }
            Message::SelectUpdates(updates_i) => {
                if let Some(updates) = &self.updates {
                    match updates
                        .get(updates_i)
                        .map(|(backend_name, package)| (*backend_name, package.clone()))
                    {
                        Some((backend_name, package)) => {
                            return self.select(
                                backend_name,
                                package.id,
                                Some(package.icon),
                                package.info,
                            );
                        }
                        None => {
                            log::error!("failed to find updates package with index {}", updates_i);
                        }
                    }
                }
            }
            Message::SelectNone => {
                self.selected_opt = None;
                return self.update_scroll();
            }
            Message::SelectCategoryResult(result_i) => {
                if let Some((_, results)) = &self.category_results {
                    match results.get(result_i) {
                        Some(result) => {
                            return self.select(
                                result.backend_name,
                                result.id.clone(),
                                result.icon_opt.clone(),
                                result.info.clone(),
                            );
                        }
                        None => {
                            log::error!("failed to find category result with index {}", result_i);
                        }
                    }
                }
            }
            Message::SelectExploreResult(explore_page, result_i) => {
                if let Some(results) = self.explore_results.get(&explore_page) {
                    match results.get(result_i) {
                        Some(result) => {
                            return self.select(
                                result.backend_name,
                                result.id.clone(),
                                result.icon_opt.clone(),
                                result.info.clone(),
                            );
                        }
                        None => {
                            log::error!(
                                "failed to find {:?} result with index {}",
                                explore_page,
                                result_i
                            );
                        }
                    }
                }
            }
            Message::SelectSearchResult(result_i) => {
                if let Some((_input, results)) = &self.search_results {
                    match results.get(result_i) {
                        Some(result) => {
                            return self.select(
                                result.backend_name,
                                result.id.clone(),
                                result.icon_opt.clone(),
                                result.info.clone(),
                            );
                        }
                        None => {
                            log::error!("failed to find search result with index {}", result_i);
                        }
                    }
                }
            }
            Message::SelectedAddonsViewMore(addons_view_more) => {
                if let Some(selected) = &mut self.selected_opt {
                    selected.addons_view_more = addons_view_more;
                }
            }
            Message::SelectedScreenshot(i, url, data) => {
                if let Some(selected) = &mut self.selected_opt {
                    if let Some(screenshot) = selected.info.screenshots.get(i) {
                        if screenshot.url == url {
                            selected
                                .screenshot_images
                                .insert(i, widget::image::Handle::from_bytes(data));
                        }
                    }
                }
            }
            Message::SelectedScreenshotShown(i) => {
                if let Some(selected) = &mut self.selected_opt {
                    selected.screenshot_shown = i;
                }
            }
            Message::ToggleUninstallPurgeData(value) => {
                self.uninstall_purge_data = value;
            }
            Message::SelectedSource(i) => {
                //TODO: show warnings if anything is not found?
                let mut next_ids = None;
                if let Some(selected) = &self.selected_opt {
                    if let Some(source) = selected.sources.get(i) {
                        next_ids = Some((
                            source.backend_name,
                            source.source_id.clone(),
                            selected.id.clone(),
                        ));
                    }
                }

                //TODO: can this be simplified?
                if let Some((backend_name, source_id, id)) = next_ids {
                    if let Some(backend) = self.backends.get(&backend_name) {
                        for appstream_cache in backend.info_caches() {
                            if appstream_cache.source_id == source_id {
                                if let Some(info) = appstream_cache.infos.get(&id) {
                                    return self.select(
                                        backend_name,
                                        id,
                                        Some(appstream_cache.icon(info)),
                                        info.clone(),
                                    );
                                }
                            }
                        }
                    }

                    // Search for installed item if appstream cache had no info (for system packages)
                    if let Some(installed) = &self.installed {
                        for (installed_backend_name, package) in installed {
                            if installed_backend_name == &backend_name
                                && package.info.source_id == source_id
                                && package.id == id
                            {
                                return self.select(
                                    backend_name,
                                    id,
                                    Some(package.icon.clone()),
                                    package.info.clone(),
                                );
                            }
                        }
                    }
                }
            }
            Message::SystemThemeModeChange(_theme_mode) => {
                return self.update_config();
            }
            Message::ToggleContextPage(context_page) => {
                //TODO: ensure context menus are closed
                if self.context_page == context_page {
                    self.core.window.show_context = !self.core.window.show_context;
                } else {
                    self.context_page = context_page;
                    self.core.window.show_context = true;
                }
            }
            Message::UpdateAll => {
                if let Some(updates) = &self.updates {
                    let mut ops = HashMap::with_capacity(self.backends.len());
                    for (backend_name, package) in updates.iter() {
                        let op = ops.entry(*backend_name).or_insert_with(|| Operation {
                            kind: OperationKind::Update,
                            backend_name: *backend_name,
                            package_ids: Vec::new(),
                            infos: Vec::new(),
                        });
                        op.package_ids.push(package.id.clone());
                        op.infos.push(package.info.clone());
                    }
                    for (_backend_name, op) in ops {
                        self.operation(op);
                    }
                }
            }
            Message::Updates(updates) => {
                self.updates = Some(updates);
                self.waiting_updates.clear();
            }
            Message::WindowClose => {
                if let Some(window_id) = self.core.main_window_id() {
                    self.core.set_main_window_id(None);
                    return Task::batch([
                        window::close(window_id),
                        Task::perform(async move { action::app(Message::MaybeExit) }, |x| x),
                    ]);
                }
            }
            Message::WindowNew => match env::current_exe() {
                Ok(exe) => match process::Command::new(&exe).spawn() {
                    Ok(_child) => {}
                    Err(err) => {
                        log::error!("failed to execute {:?}: {}", exe, err);
                    }
                },
                Err(err) => {
                    log::error!("failed to get current executable path: {}", err);
                }
            },
            Message::SelectPlacement(selection) => {
                self.applet_placement_buttons.activate(selection);
            }
            #[cfg(not(feature = "wayland"))]
            Message::PlaceApplet(id) => {
                log::error!(
                    "cannot place applet {:?}, not compiled with wayland feature",
                    id
                );
            }
            #[cfg(feature = "wayland")]
            Message::PlaceApplet(id) => {
                self.dialog_pages.pop_front();

                // Panel or Dock specific references
                let panel_info = if Some(self.applet_placement_buttons.active())
                    == self.applet_placement_buttons.entity_at(1)
                {
                    ("Dock", "cosmic-settings dock-applet")
                } else {
                    ("Panel", "cosmic-settings panel-applet")
                };

                // Load in panel or dock configs for adding new applet
                let panel_config_helper = CosmicPanelConfig::cosmic_config(panel_info.0).ok();
                let mut applet_config =
                    panel_config_helper
                        .as_ref()
                        .and_then(|panel_config_helper| {
                            let panel_config =
                                CosmicPanelConfig::get_entry(panel_config_helper).ok()?;
                            (panel_config.name == panel_info.0).then_some(panel_config)
                        });
                let Some(applet_config) = applet_config.as_mut() else {
                    return Task::none();
                };

                // check if the applet is already added to the panel
                let applet_id = id.raw().to_owned();
                let mut applet_exists = false;
                if let Some(center) = applet_config.plugins_center.as_ref() {
                    if center.iter().any(|a: &String| a.as_str() == applet_id) {
                        applet_exists = true;
                    }
                }
                if let Some(wings) = applet_config.plugins_wings.as_ref() {
                    if wings
                        .0
                        .iter()
                        .chain(wings.1.iter())
                        .any(|a: &String| a.as_str() == applet_id)
                    {
                        applet_exists = true;
                    }
                }

                // if applet doesn't already exist, continue adding
                if !applet_exists {
                    // add applet to the end of the left wing (matching the applet settings behaviour)
                    let list = if let Some((list, _)) = applet_config.plugins_wings.as_mut() {
                        list
                    } else {
                        applet_config.plugins_wings = Some((Vec::new(), Vec::new()));
                        &mut applet_config.plugins_wings.as_mut().unwrap().0
                    };
                    list.push(id.raw().to_string());

                    // save config
                    if let Some(save_helper) = panel_config_helper.as_ref() {
                        if let Err(e) = applet_config.write_entry(save_helper) {
                            log::error!("Failed to save applet: {:?}", e);
                        }
                    } else {
                        log::error!("No panel config helper. Failed to save applet.");
                    };
                }

                // launch the applet settings
                let settings_desktop_id = "com.system76.CosmicSettings";
                let exec = panel_info.1;
                return Task::perform(
                    async move {
                        tokio::task::spawn_blocking(move || Some((exec, settings_desktop_id)))
                            .await
                            .unwrap_or(None)
                    },
                    |result| {
                        #[cfg(feature = "desktop")]
                        if let Some((exec, settings_desktop_id)) = result {
                            tokio::spawn(async move {
                                cosmic::desktop::spawn_desktop_exec(
                                    &exec,
                                    Vec::<(&str, &str)>::new(),
                                    Some(settings_desktop_id),
                                    false,
                                )
                                .await;
                            });
                        }
                        action::none()
                    },
                );
            }
        }

        Task::none()
    }
}
