// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::cmp;
use std::sync::Arc;

use cosmic::{
    Element, cosmic_theme,
    iced::{Alignment, Length, Size},
    theme, widget,
};
use rayon::prelude::*;

use crate::app_id::AppId;
use crate::app_info::{AppInfo, AppProvide, AppUrl};
use crate::backend::{BackendName, Package};
use crate::config::AppTheme;
use crate::explore::ExplorePage;
use crate::fl;
use crate::icon_cache::icon_cache_handle;
use crate::localize::LANGUAGE_SORTER;
use crate::nav::NavPage;
use crate::operation::OperationKind;
use crate::search::{GridMetrics, SearchResult};
use crate::{
    App, AppEntry, ContextPage, DialogPage, ICON_SIZE_DETAILS, ICON_SIZE_PACKAGE, MAX_RESULTS,
    Message, SelectedSource, SourceKind,
};

pub fn package_card_view<'a>(
    info: &'a AppInfo,
    icon_opt: Option<&'a widget::icon::Handle>,
    controls: Vec<Element<'a, Message>>,
    top_controls: Option<Vec<Element<'a, Message>>>,
    spacing: &cosmic_theme::Spacing,
    width: usize,
) -> Element<'a, Message> {
    let height = 20.0 + 28.0 + 32.0 + 3.0 * spacing.space_xxs as f32;
    let top_row_cap = 1 + top_controls
        .as_deref()
        .map(|elements| 1 + elements.len())
        .unwrap_or_default();
    let column = widget::column::with_children(vec![
        widget::row::with_capacity(top_row_cap)
            .push(widget::column::with_children(vec![
                widget::text::body(&info.name)
                    .height(20.0)
                    .width(width as f32 - 180.0)
                    .into(),
                widget::text::caption(&info.summary)
                    .height(28.0)
                    .width(width as f32 - 180.0)
                    .into(),
            ]))
            .push_maybe(top_controls.is_some().then_some(widget::horizontal_space()))
            .extend(top_controls.unwrap_or_default())
            .into(),
        widget::Space::with_height(Length::Fixed(spacing.space_xxs.into())).into(),
        widget::row::with_children(controls)
            .height(32.0)
            .spacing(spacing.space_xs)
            .into(),
    ]);

    let icon: Element<_> = match icon_opt {
        Some(icon) => widget::icon::icon(icon.clone())
            .size(ICON_SIZE_PACKAGE)
            .into(),
        None => widget::Space::with_width(ICON_SIZE_PACKAGE as f32).into(),
    };

    widget::container(
        widget::row()
            .push(icon)
            .push(column)
            .align_y(Alignment::Center)
            .spacing(spacing.space_s),
    )
    .align_y(Alignment::Center)
    .width(width as f32)
    .height(height)
    .padding([spacing.space_xxs, spacing.space_s])
    .class(theme::Container::Card)
    .into()
}

impl Package {
    pub fn grid_metrics(spacing: &cosmic_theme::Spacing, width: usize) -> GridMetrics {
        GridMetrics::new(width, 320 + 2 * spacing.space_s as usize, spacing.space_xxs)
    }

    pub fn card_view<'a>(
        &'a self,
        controls: Vec<Element<'a, Message>>,
        top_controls: Option<Vec<Element<'a, Message>>>,
        spacing: &cosmic_theme::Spacing,
        width: usize,
    ) -> Element<'a, Message> {
        package_card_view(
            &self.info,
            Some(&self.icon),
            controls,
            top_controls,
            spacing,
            width,
        )
    }
}

impl App {
    fn selected_buttons(
        &self,
        selected_backend_name: BackendName,
        selected_id: &AppId,
        selected_info: &Arc<AppInfo>,
        addon: bool,
    ) -> Vec<Element<'_, Message>> {
        //TODO: more efficient checks
        let mut waiting_refresh = false;
        for (backend_name, source_id, package_id) in self
            .waiting_installed
            .iter()
            .chain(self.waiting_updates.iter())
        {
            if *backend_name == selected_backend_name
                && source_id == &selected_info.source_id
                && package_id == selected_id
            {
                waiting_refresh = true;
                break;
            }
        }
        let is_installed = self.is_installed(selected_backend_name, selected_id, selected_info);
        let applet_provide = AppProvide::Id("com.system76.CosmicApplet".to_string());
        let mut update_opt = None;
        if let Some(updates) = &self.updates {
            for (backend_name, package) in updates {
                if *backend_name == selected_backend_name
                    && package.info.source_id == selected_info.source_id
                    && &package.id == selected_id
                {
                    update_opt = Some(Message::Operation(
                        OperationKind::Update,
                        *backend_name,
                        package.id.clone(),
                        package.info.clone(),
                    ));
                    break;
                }
            }
        }
        let mut progress_opt = None;
        for (_id, (op, progress)) in self.pending_operations.iter() {
            if op.backend_name == selected_backend_name
                && op
                    .infos
                    .iter()
                    .any(|info| info.source_id == selected_info.source_id)
                && op
                    .package_ids
                    .iter()
                    .any(|package_id| package_id == selected_id)
            {
                progress_opt = Some(*progress);
                break;
            }
        }

        let mut buttons = Vec::with_capacity(2);
        if let Some(progress) = progress_opt {
            //TODO: get height from theme?
            buttons.push(
                widget::progress_bar(0.0..=100.0, progress)
                    .height(Length::Fixed(4.0))
                    .into(),
            )
        } else if waiting_refresh {
            // Do not show buttons while waiting for refresh
        } else if is_installed {
            //TODO: what if there are multiple desktop IDs?
            if let Some(desktop_id) = selected_info.desktop_ids.first() {
                if selected_info.provides.contains(&applet_provide) {
                    buttons.push(
                        widget::button::suggested(fl!("place-on-desktop"))
                            .on_press(Message::DialogPage(DialogPage::Place(selected_id.clone())))
                            .into(),
                    );
                } else {
                    buttons.push(
                        widget::button::suggested(fl!("open"))
                            .on_press(Message::OpenDesktopId(desktop_id.clone()))
                            .into(),
                    );
                }
            }
            if let Some(update) = update_opt {
                buttons.push(
                    widget::button::standard(fl!("update"))
                        .on_press(update)
                        .into(),
                );
            }
            if !selected_id.is_system() {
                buttons.push(
                    widget::button::standard(fl!("uninstall"))
                        .on_press(Message::DialogPage(DialogPage::Uninstall(
                            selected_backend_name,
                            selected_id.clone(),
                            selected_info.clone(),
                        )))
                        .into(),
                );
            }
        } else {
            buttons.push(
                if addon {
                    widget::button::standard(fl!("install"))
                } else {
                    widget::button::suggested(fl!("install"))
                }
                .on_press(Message::Operation(
                    OperationKind::Install,
                    selected_backend_name,
                    selected_id.clone(),
                    selected_info.clone(),
                ))
                .into(),
            )
        }

        buttons
    }

    pub fn selected_sources(
        &self,
        backend_name: BackendName,
        id: &AppId,
        info: &AppInfo,
    ) -> Vec<SelectedSource> {
        let mut sources = Vec::new();
        match self.apps.get(id) {
            Some(infos) => {
                for AppEntry {
                    backend_name,
                    info,
                    installed,
                } in infos.iter()
                {
                    sources.push(SelectedSource::new(*backend_name, info, *installed));
                }
            }
            None => {
                //TODO: warning?
                let installed = self.is_installed(backend_name, id, info);
                sources.push(SelectedSource::new(backend_name, info, installed));
            }
        }
        sources
    }

    pub fn selected_addons(
        &self,
        backend_name: BackendName,
        id: &AppId,
        info: &AppInfo,
    ) -> Vec<(AppId, Arc<AppInfo>)> {
        let mut addons = Vec::new();
        if let Some(backend) = self.backends.get(&backend_name) {
            for appstream_cache in backend.info_caches() {
                if appstream_cache.source_id == info.source_id {
                    if let Some(ids) = appstream_cache.addons.get(id) {
                        for id in ids {
                            if let Some(info) = appstream_cache.infos.get(id) {
                                addons.push((id.clone(), info.clone()));
                            }
                        }
                    }
                }
            }
        }
        addons.par_sort_unstable_by(|a, b| {
            match b.1.monthly_downloads.cmp(&a.1.monthly_downloads) {
                cmp::Ordering::Equal => LANGUAGE_SORTER.compare(&a.1.name, &b.1.name),
                ordering => ordering,
            }
        });
        addons
    }

    pub fn settings(&self) -> Element<'_, Message> {
        let app_theme_selected = match self.config.app_theme {
            AppTheme::Dark => 1,
            AppTheme::Light => 2,
            AppTheme::System => 0,
        };
        widget::settings::view_column(vec![
            widget::settings::section()
                .title(fl!("appearance"))
                .add(
                    widget::settings::item::builder(fl!("theme")).control(widget::dropdown(
                        &self.app_themes,
                        Some(app_theme_selected),
                        move |index| {
                            Message::AppTheme(match index {
                                1 => AppTheme::Dark,
                                2 => AppTheme::Light,
                                _ => AppTheme::System,
                            })
                        },
                    )),
                )
                .into(),
        ])
        .into()
    }

    pub fn view_responsive(&self, size: Size) -> Element<'_, Message> {
        self.size.set(Some(size));
        let spacing = theme::active().cosmic().spacing;
        let cosmic_theme::Spacing {
            space_l,
            space_m,
            space_s,
            space_xs,
            space_xxs,
            space_xxxs,
            ..
        } = spacing;
        let grid_width = (size.width - 2.0 * space_s as f32).floor().max(0.0) as usize;
        match &self.selected_opt {
            Some(selected) => {
                let mut selected_source = None;
                for (i, source) in selected.sources.iter().enumerate() {
                    if source.backend_name == selected.backend_name
                        && source.source_id == selected.info.source_id
                    {
                        selected_source = Some(i);
                        break;
                    }
                }

                let mut column = widget::column::with_capacity(2)
                    .padding([0, space_s, space_m, space_s])
                    .spacing(space_m)
                    .width(Length::Fill);
                column = column.push(
                    //TODO: describe where we are going back to
                    widget::button::text(fl!("back"))
                        .leading_icon(icon_cache_handle("go-previous-symbolic", 16))
                        .on_press(Message::SelectNone),
                );

                let buttons = self.selected_buttons(
                    selected.backend_name,
                    &selected.id,
                    &selected.info,
                    false,
                );
                column = column.push(
                    widget::row::with_children(vec![
                        match &selected.icon_opt {
                            Some(icon) => widget::icon::icon(icon.clone())
                                .size(ICON_SIZE_DETAILS)
                                .into(),
                            None => {
                                widget::Space::with_width(Length::Fixed(ICON_SIZE_DETAILS as f32))
                                    .into()
                            }
                        },
                        widget::column::with_children(vec![
                            widget::text::title2(&selected.info.name).into(),
                            widget::text(&selected.info.summary).into(),
                            widget::Space::with_height(Length::Fixed(space_s.into())).into(),
                            widget::row::with_children(buttons).spacing(space_xs).into(),
                        ])
                        .into(),
                    ])
                    .align_y(Alignment::Center)
                    .spacing(space_m),
                );

                let sources_widget = widget::column::with_children(vec![if selected.sources.len()
                    == 1
                {
                    widget::text(selected.sources[0].as_ref()).into()
                } else {
                    widget::dropdown(&selected.sources, selected_source, Message::SelectedSource)
                        .into()
                }])
                .align_x(Alignment::Center)
                .width(Length::Fill);
                let developers_widget = widget::column::with_children(vec![
                    if selected.info.developer_name.is_empty() {
                        widget::text::heading(fl!(
                            "app-developers",
                            app = selected.info.name.as_str()
                        ))
                        .into()
                    } else {
                        widget::text::heading(&selected.info.developer_name).into()
                    },
                    widget::text::body(fl!("developer")).into(),
                ])
                .align_x(Alignment::Center)
                .width(Length::Fill);
                let downloads_widget = (selected.info.monthly_downloads > 0).then(|| {
                    widget::column::with_children(vec![
                        widget::text::heading(selected.info.monthly_downloads.to_string()).into(),
                        //TODO: description of what this means?
                        widget::text::body(fl!("monthly-downloads")).into(),
                    ])
                    .align_x(Alignment::Center)
                    .width(Length::Fill)
                });
                if grid_width < 416 {
                    let size = 4 + if downloads_widget.is_some() { 3 } else { 0 };
                    let downloads_widget_space = downloads_widget
                        .is_some()
                        .then(widget::divider::horizontal::default);
                    column = column.push(
                        widget::column::with_capacity(size)
                            .push(widget::divider::horizontal::default())
                            .push(sources_widget)
                            .push(widget::divider::horizontal::default())
                            .push(developers_widget)
                            .push(widget::divider::horizontal::default())
                            .push_maybe(downloads_widget)
                            .push_maybe(downloads_widget_space)
                            .spacing(space_xxs),
                    );
                } else {
                    let row_size = 4 + if downloads_widget.is_some() { 2 } else { 0 };
                    let downloads_widget_space = downloads_widget
                        .is_some()
                        .then(|| widget::divider::vertical::default().height(Length::Fixed(32.0)));
                    column = column.push(
                        widget::column::with_children(vec![
                            widget::divider::horizontal::default().into(),
                            widget::row::with_capacity(row_size)
                                .push(sources_widget)
                                .push(
                                    widget::divider::vertical::default()
                                        .height(Length::Fixed(32.0)),
                                )
                                .push(developers_widget)
                                .push_maybe(downloads_widget_space)
                                .push_maybe(downloads_widget)
                                .align_y(Alignment::Center)
                                .into(),
                            widget::divider::horizontal::default().into(),
                        ])
                        .spacing(space_xxs),
                    );
                }
                //TODO: proper image scroller
                if let Some(screenshot) = selected.info.screenshots.get(selected.screenshot_shown) {
                    let image_height = Length::Fixed(320.0);
                    let mut row = widget::row::with_capacity(3).align_y(Alignment::Center);
                    {
                        let mut button = widget::button::icon(
                            widget::icon::from_name("go-previous-symbolic").size(16),
                        );
                        let index = selected.screenshot_shown.checked_sub(1).unwrap_or_else(|| {
                            selected
                                .info
                                .screenshots
                                .len()
                                .checked_sub(1)
                                .unwrap_or_default()
                        });
                        if index != selected.screenshot_shown {
                            button = button.on_press(Message::SelectedScreenshotShown(index));
                        }
                        row = row.push(button);
                    }
                    let image_element = if let Some(image) =
                        selected.screenshot_images.get(&selected.screenshot_shown)
                    {
                        widget::container(widget::image(image.clone()))
                            .center_x(Length::Fill)
                            .center_y(image_height)
                            .into()
                    } else {
                        widget::Space::new(Length::Fill, image_height).into()
                    };
                    row = row.push(
                        widget::column::with_children(vec![
                            image_element,
                            widget::text::caption(&screenshot.caption).into(),
                        ])
                        .align_x(Alignment::Center),
                    );
                    {
                        let mut button = widget::button::icon(
                            widget::icon::from_name("go-next-symbolic").size(16),
                        );
                        let index =
                            if selected.screenshot_shown + 1 == selected.info.screenshots.len() {
                                0
                            } else {
                                selected.screenshot_shown + 1
                            };
                        if index != selected.screenshot_shown {
                            button = button.on_press(Message::SelectedScreenshotShown(index));
                        }
                        row = row.push(button);
                    }
                    column = column.push(row);
                }
                column = column.push(widget::text::body(&selected.info.description));

                if !selected.addons.is_empty() {
                    let mut addon_col = widget::column::with_capacity(2).spacing(space_xxxs);
                    addon_col = addon_col.push(widget::text::title4(fl!("addons")));
                    let mut list = widget::list_column()
                        .divider_padding(0)
                        .list_item_padding([space_xxs, 0])
                        .style(theme::Container::Transparent);
                    let addon_cnt = selected.addons.len();
                    let take = if selected.addons_view_more {
                        addon_cnt
                    } else {
                        4
                    };
                    for (addon_id, addon_info) in selected.addons.iter().take(take) {
                        let buttons = self.selected_buttons(
                            selected.backend_name,
                            addon_id,
                            addon_info,
                            true,
                        );
                        list = list.add(
                            widget::settings::item::builder(&addon_info.name)
                                .description(&addon_info.summary)
                                .control(widget::row::with_children(buttons).spacing(space_xs)),
                        );
                    }
                    if addon_cnt > 4 && !selected.addons_view_more {
                        list = list.add(
                            widget::button::text(fl!("view-more"))
                                .on_press(Message::SelectedAddonsViewMore(true)),
                        );
                    }
                    addon_col = addon_col.push(list);
                    column = column.push(addon_col);
                }

                // Show the first (latest) release only
                if let Some(release) = selected.info.releases.first() {
                    let mut release_col = widget::column::with_capacity(2).spacing(space_xxxs);
                    release_col = release_col.push(widget::text::title4(fl!(
                        "version",
                        version = release.version.as_str()
                    )));
                    if let Some(timestamp) = release.timestamp {
                        if let Some(utc) =
                            chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
                        {
                            let local = chrono::DateTime::<chrono::Local>::from(utc);
                            release_col = release_col.push(widget::text::body(format!(
                                "{}",
                                local.format("%b %-d, %-Y")
                            )));
                        }
                    }
                    if let Some(description) = &release.description {
                        release_col = release_col.push(widget::text::body(description));
                    }
                    column = column.push(release_col);
                }

                if let Some(license) = &selected.info.license_opt {
                    let mut license_col = widget::column::with_capacity(2).spacing(space_xxxs);
                    license_col = license_col.push(widget::text::title4(fl!("licenses")));
                    match spdx::Expression::parse_mode(license, spdx::ParseMode::LAX) {
                        Ok(expr) => {
                            for item in expr.requirements() {
                                match &item.req.license {
                                    spdx::LicenseItem::Spdx { id, .. } => {
                                        license_col =
                                            license_col.push(widget::text::body(id.full_name));
                                    }
                                    spdx::LicenseItem::Other { lic_ref, .. } => {
                                        let mut parts = lic_ref.splitn(2, '=');
                                        parts.next();
                                        if let Some(url) = parts.next() {
                                            license_col = license_col.push(
                                                widget::button::link(fl!("proprietary"))
                                                    .on_press(Message::LaunchUrl(url.to_string()))
                                                    .padding(0),
                                            )
                                        } else {
                                            license_col = license_col
                                                .push(widget::text::body(fl!("proprietary")));
                                        }
                                    }
                                }
                            }
                        }
                        Err(_) => {
                            license_col = license_col.push(widget::text::body(license));
                        }
                    }
                    column = column.push(license_col);
                }

                if !selected.info.urls.is_empty() {
                    let mut url_items = Vec::with_capacity(selected.info.urls.len());
                    for app_url in &selected.info.urls {
                        let (name, url) = match app_url {
                            AppUrl::BugTracker(url) => (fl!("bug-tracker"), url),
                            AppUrl::Contact(url) => (fl!("contact"), url),
                            AppUrl::Donation(url) => (fl!("donation"), url),
                            AppUrl::Faq(url) => (fl!("faq"), url),
                            AppUrl::Help(url) => (fl!("help"), url),
                            AppUrl::Homepage(url) => (fl!("homepage"), url),
                            AppUrl::Translate(url) => (fl!("translate"), url),
                        };
                        url_items.push(
                            widget::button::link(name)
                                .on_press(Message::LaunchUrl(url.to_string()))
                                .padding(0)
                                .into(),
                        );
                    }
                    if grid_width < 416 {
                        column = column
                            .push(widget::column::with_children(url_items).spacing(space_xxxs));
                    } else {
                        column = column.push(
                            widget::row::with_children(url_items)
                                .spacing(space_s)
                                .align_y(Alignment::Center),
                        );
                    }
                }

                column.into()
            }
            None => match &self.search_results {
                Some((input, results)) => {
                    //TODO: paging or dynamic load
                    let results_len = cmp::min(results.len(), MAX_RESULTS);

                    let mut column = widget::column::with_capacity(2)
                        .padding([0, space_s, space_m, space_s])
                        .spacing(space_xxs)
                        .width(Length::Fill);
                    //TODO: back button?
                    if results.is_empty() {
                        column = column.push(widget::text::body(fl!(
                            "no-results",
                            search = input.as_str()
                        )));
                    }
                    column = column.push(SearchResult::grid_view(
                        &results[..results_len],
                        spacing,
                        grid_width,
                        Message::SelectSearchResult,
                    ));
                    column.into()
                }
                None => match self
                    .nav_model
                    .active_data::<NavPage>()
                    .map_or(NavPage::default(), |nav_page| *nav_page)
                {
                    NavPage::Explore => {
                        match self.explore_page_opt {
                            Some(explore_page) => {
                                let mut column = widget::column::with_capacity(3)
                                    .padding([0, space_s, space_m, space_s])
                                    .spacing(space_xxs)
                                    .width(Length::Fill);
                                column = column.push(
                                    widget::button::text(NavPage::Explore.title())
                                        .leading_icon(icon_cache_handle("go-previous-symbolic", 16))
                                        .on_press(Message::ExplorePage(None)),
                                );
                                column = column.push(widget::text::title4(explore_page.title()));
                                //TODO: ensure explore_page matches
                                match self.explore_results.get(&explore_page) {
                                    Some(results) => {
                                        //TODO: paging or dynamic load
                                        let results_len = cmp::min(results.len(), MAX_RESULTS);

                                        if results.is_empty() {
                                            //TODO: no results message?
                                        }
                                        column = column.push(SearchResult::grid_view(
                                            &results[..results_len],
                                            spacing,
                                            grid_width,
                                            move |result_i| {
                                                Message::SelectExploreResult(explore_page, result_i)
                                            },
                                        ));
                                    }
                                    None => {
                                        //TODO: loading message?
                                    }
                                }
                                column.into()
                            }
                            None => {
                                let explore_pages = ExplorePage::all();
                                let mut column =
                                    widget::column::with_capacity(explore_pages.len() * 2)
                                        .padding([0, space_s, space_m, space_s])
                                        .spacing(space_xxs)
                                        .width(Length::Fill);
                                for explore_page in explore_pages.iter() {
                                    //TODO: ensure explore_page matches
                                    match self.explore_results.get(explore_page) {
                                        Some(results) if !results.is_empty() => {
                                            let GridMetrics { cols, .. } =
                                                SearchResult::grid_metrics(&spacing, grid_width);

                                            let max_results = match cols {
                                                1 => 4,
                                                2 => 8,
                                                3 => 9,
                                                _ => cols * 2,
                                            };

                                            //TODO: adjust results length based on app size?
                                            let results_len = cmp::min(results.len(), max_results);

                                            column = column.push(widget::row::with_children(vec![
                                                widget::text::title4(explore_page.title()).into(),
                                                widget::horizontal_space().into(),
                                                widget::button::text(fl!("see-all"))
                                                    .trailing_icon(icon_cache_handle(
                                                        "go-next-symbolic",
                                                        16,
                                                    ))
                                                    .on_press(Message::ExplorePage(Some(
                                                        *explore_page,
                                                    )))
                                                    .into(),
                                            ]));

                                            column = column.push(SearchResult::grid_view(
                                                &results[..results_len],
                                                spacing,
                                                grid_width,
                                                |result_i| {
                                                    Message::SelectExploreResult(
                                                        *explore_page,
                                                        result_i,
                                                    )
                                                },
                                            ));
                                        }
                                        _ => {}
                                    }
                                }
                                column.into()
                            }
                        }
                    }
                    NavPage::Installed => {
                        let mut column = widget::column::with_capacity(3)
                            .padding([0, space_s, space_m, space_s])
                            .spacing(space_xxs)
                            .width(Length::Fill);
                        column = column.push(widget::text::title2(NavPage::Installed.title()));
                        match &self.installed_results {
                            Some(installed) => {
                                if installed.is_empty() {
                                    column =
                                        column.push(widget::text(fl!("no-installed-applications")));
                                }

                                let GridMetrics {
                                    cols,
                                    item_width,
                                    column_spacing,
                                } = Package::grid_metrics(&spacing, grid_width);
                                let mut grid = widget::grid();
                                let mut col = 0;
                                for (installed_i, result) in installed.iter().enumerate() {
                                    if col >= cols {
                                        grid = grid.insert_row();
                                        col = 0;
                                    }
                                    let mut buttons = Vec::with_capacity(1);
                                    if let Some(desktop_id) = result.info.desktop_ids.first() {
                                        buttons.push(
                                            widget::button::standard(fl!("open"))
                                                .on_press(Message::OpenDesktopId(
                                                    desktop_id.clone(),
                                                ))
                                                .into(),
                                        );
                                    } else {
                                        buttons.push(
                                            widget::Space::with_height(Length::Shrink).into(),
                                        );
                                    }
                                    grid = grid.push(
                                        widget::mouse_area(package_card_view(
                                            &result.info,
                                            result.icon_opt.as_ref(),
                                            buttons,
                                            None,
                                            &spacing,
                                            item_width,
                                        ))
                                        .on_press(Message::SelectInstalled(installed_i)),
                                    );
                                    col += 1;
                                }
                                column = column.push(
                                    grid.column_spacing(column_spacing)
                                        .row_spacing(column_spacing),
                                );
                            }
                            None => {
                                //TODO: loading message?
                            }
                        }
                        column.into()
                    }
                    //TODO: reduce duplication
                    NavPage::Updates => {
                        let mut column = widget::column::with_capacity(3)
                            .padding([0, space_s, space_m, space_s])
                            .spacing(space_xxs)
                            .width(Length::Fill);
                        match &self.updates {
                            Some(updates) => {
                                if updates.is_empty() {
                                    column = column
                                        .push(widget::text::title2(NavPage::Updates.title()))
                                        .push(
                                            widget::column::with_capacity(2)
                                                .spacing(space_s)
                                                .padding([space_l, 0])
                                                .width(Length::Fill)
                                                .align_x(Alignment::Center)
                                                .push(widget::text::body(fl!("no-updates")))
                                                .push(
                                                    widget::button::standard(fl!(
                                                        "check-for-updates"
                                                    ))
                                                    .on_press(Message::CheckUpdates),
                                                ),
                                        );
                                } else {
                                    column = column.push(widget::flex_row(vec![
                                        widget::text::title2(NavPage::Updates.title()).into(),
                                        widget::horizontal_space().width(Length::Fill).into(),
                                        widget::row::with_capacity(2)
                                            .spacing(space_xxs)
                                            .push(
                                                widget::button::standard(fl!("check-for-updates"))
                                                    .on_press(Message::CheckUpdates),
                                            )
                                            .push(
                                                widget::button::standard(fl!("update-all"))
                                                    .on_press(Message::UpdateAll),
                                            )
                                            .into(),
                                    ]));
                                }

                                let GridMetrics {
                                    cols,
                                    item_width,
                                    column_spacing,
                                } = Package::grid_metrics(&spacing, grid_width);
                                let mut grid = widget::grid();
                                let mut col = 0;
                                for (updates_i, (backend_name, package)) in
                                    updates.iter().enumerate()
                                {
                                    let mut waiting_refresh = false;
                                    for (other_backend_name, source_id, package_id) in self
                                        .waiting_installed
                                        .iter()
                                        .chain(self.waiting_updates.iter())
                                    {
                                        if other_backend_name == backend_name
                                            && source_id == &package.info.source_id
                                            && package_id == &package.id
                                        {
                                            waiting_refresh = true;
                                            break;
                                        }
                                    }
                                    let mut progress_opt = None;
                                    for (_id, (op, progress)) in self.pending_operations.iter() {
                                        if &op.backend_name == backend_name
                                            && op.infos.iter().any(|info| {
                                                info.source_id == package.info.source_id
                                            })
                                            && op
                                                .package_ids
                                                .iter()
                                                .any(|package_id| package_id == &package.id)
                                        {
                                            progress_opt = Some(*progress);
                                            break;
                                        }
                                    }
                                    let controls = if let Some(progress) = progress_opt {
                                        vec![
                                            widget::progress_bar(0.0..=100.0, progress)
                                                .height(Length::Fixed(4.0))
                                                .into(),
                                        ]
                                    } else if waiting_refresh {
                                        vec![]
                                    } else {
                                        vec![
                                            widget::button::standard(fl!("update"))
                                                .on_press(Message::Operation(
                                                    OperationKind::Update,
                                                    *backend_name,
                                                    package.id.clone(),
                                                    package.info.clone(),
                                                ))
                                                .into(),
                                        ]
                                    };
                                    let top_controls = Some(vec![
                                        widget::button::icon(widget::icon::from_name(
                                            "help-info-symbolic",
                                        ))
                                        .on_press(Message::ToggleContextPage(
                                            ContextPage::ReleaseNotes(
                                                updates_i,
                                                package.info.name.clone(),
                                            ),
                                        ))
                                        .into(),
                                    ]);
                                    if col >= cols {
                                        grid = grid.insert_row();
                                        col = 0;
                                    }
                                    grid = grid.push(
                                        widget::mouse_area(package.card_view(
                                            controls,
                                            top_controls,
                                            &spacing,
                                            item_width,
                                        ))
                                        .on_press(Message::SelectUpdates(updates_i)),
                                    );
                                    col += 1;
                                }
                                column = column.push(
                                    grid.column_spacing(column_spacing)
                                        .row_spacing(column_spacing),
                                );
                            }
                            None => {
                                column = column
                                    .push(widget::text::title2(NavPage::Updates.title()))
                                    .push(
                                        widget::column::with_capacity(2)
                                            .spacing(space_s)
                                            .padding([space_l, 0])
                                            .width(Length::Fill)
                                            .align_x(Alignment::Center)
                                            /*.push(
                                                widget::progress_bar(0.0..=100.0, progress)
                                                    .height(Length::Fixed(4.0))
                                                    .width(Length::Fixed(446.0)),
                                            )*/
                                            .push(widget::text(fl!("checking-for-updates"))),
                                    );
                            }
                        }
                        column.into()
                    }
                    //TODO: reduce duplication
                    nav_page => {
                        let mut column = widget::column::with_capacity(3)
                            .padding([0, space_s, space_m, space_s])
                            .spacing(space_xxs)
                            .width(Length::Fill);
                        column = column.push(widget::text::title2(nav_page.title()));
                        if matches!(nav_page, NavPage::Applets) {
                            let sources = self.sources();
                            if !sources.is_empty()
                                && sources.iter().any(|source| {
                                    matches!(
                                        source.kind,
                                        SourceKind::Recommended { enabled: false, .. }
                                    )
                                })
                            {
                                column = column.push(
                                    widget::column::with_children(vec![
                                        widget::Space::with_height(space_m).into(),
                                        widget::text(fl!("enable-flathub-cosmic")).into(),
                                        widget::Space::with_height(space_m).into(),
                                        widget::button::standard(fl!("manage-repositories"))
                                            .on_press(Message::ToggleContextPage(
                                                ContextPage::Repositories,
                                            ))
                                            .into(),
                                        widget::Space::with_height(space_l).into(),
                                    ])
                                    .align_x(Alignment::Center)
                                    .width(Length::Fill),
                                );
                            }
                        }
                        //TODO: ensure category matches?
                        match &self.category_results {
                            Some((_, results)) => {
                                //TODO: paging or dynamic load
                                let results_len = cmp::min(results.len(), MAX_RESULTS);

                                if results.is_empty() {
                                    //TODO: no results message?
                                }

                                column = column.push(SearchResult::grid_view(
                                    &results[..results_len],
                                    spacing,
                                    grid_width,
                                    Message::SelectCategoryResult,
                                ));
                            }
                            None => {
                                //TODO: loading message?
                            }
                        }
                        column.into()
                    }
                },
            },
        }
    }
}
