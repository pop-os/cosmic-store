// Copyright 2023 System76 <info@system76.com>
// SPDX-License-Identifier: GPL-3.0-only

use std::sync::Arc;

use cosmic::{
    Apply, Element,
    iced::{
        Alignment, Color, Length, Size,
        core::text::{Ellipsize, EllipsizeHeightLimit},
    },
    theme, widget,
};

use crate::app_info::{AppInfo, AppProvide, AppUrl};
use crate::backend::{BackendName, Package};
use crate::config::AppTheme;
use crate::explore::ExplorePage;
use crate::fl;
use crate::icon_cache::icon_cache_handle;
use crate::localize::LANGUAGE_SORTER;
use crate::nav::NavPage;
use crate::operation::OperationKind;
use crate::search::SearchResult;
use crate::{
    App, ContextPage, DialogPage, ICON_SIZE_CARD, ICON_SIZE_DETAILS, MAX_RESULTS, Message,
    Selected, SelectedSource, SourceKind,
};
use crate::{CARD_TEXT_WIDTH, app_id::AppId};

pub struct GridMetrics {
    pub cols: usize,
    pub item_width: usize,
    pub column_spacing: u16,
}

impl GridMetrics {
    pub fn new(width: usize) -> Self {
        let spacing = theme::spacing();
        let column_spacing = spacing.space_m;
        let min_width =
            (ICON_SIZE_CARD + 2 * spacing.space_xxs + spacing.space_xs + CARD_TEXT_WIDTH) as usize;
        let width_m1 = width.saturating_sub(min_width);
        let cols_m1 = width_m1 / (min_width + column_spacing as usize);
        let cols = cols_m1 + 1;
        let item_width = width.saturating_sub(cols_m1 * column_spacing as usize) / cols;

        Self {
            cols,
            item_width,
            column_spacing,
        }
    }

    pub fn build_grid<'a, I>(&self, items: I) -> Element<'a, Message>
    where
        I: IntoIterator<Item = Element<'a, Message>>,
    {
        let mut grid = widget::grid();
        let mut col = 0;
        for item in items {
            if col >= self.cols {
                grid = grid.insert_row();
                col = 0;
            }
            grid = grid.push(item);
            col += 1;
        }

        grid.column_spacing(self.column_spacing)
            .row_spacing(self.column_spacing)
            .into()
    }
}

/// Card height including padding
pub fn card_height() -> f32 {
    (ICON_SIZE_CARD + 2 * theme::spacing().space_xxs).into()
}

pub fn format_downloads(x: u64) -> String {
    for &(threshold, suffix) in &[
        (1_000_000_000, " B"),
        (100_000_000, "00 M"),
        (10_000_000, "0 M"),
        (1_000_000, " M"),
        (100_000, "00 K"),
        (10_000, "0 K"),
        (1_000, " K"),
    ] {
        if x > threshold {
            return format!("{}{}", x / threshold, suffix);
        }
    }
    format!("{}", x)
}

pub fn card_tags<'a>(info: &'a AppInfo) -> Element<'a, Message> {
    let spacing = theme::spacing();
    let mut tags = Vec::with_capacity(3);
    if info.monthly_downloads > 0 {
        tags.push(
            widget::row::with_children([
                widget::icon::from_name("folder-download-symbolic").into(),
                widget::text::caption(format_downloads(info.monthly_downloads))
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                    .into(),
            ])
            .align_y(Alignment::Center)
            .spacing(spacing.space_xxxs)
            .into(),
        );
    }
    if !info.developer_name.is_empty() {
        if !tags.is_empty() {
            tags.push(widget::divider::vertical::default().into());
        }
        tags.push(
            widget::row::with_children([
                widget::icon::from_name("system-users-symbolic").into(),
                widget::text::caption(&info.developer_name)
                    .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
                    .into(),
            ])
            .align_y(Alignment::Center)
            .spacing(spacing.space_xxxs)
            .into(),
        );
    }
    widget::row::with_children(tags)
        .align_y(Alignment::Center)
        .height(17)
        .spacing(spacing.space_xs)
        .into()
}

pub fn card_view<'a>(
    info: &'a AppInfo,
    icon_opt: Option<&'a widget::icon::Handle>,
    footer: Element<'a, Message>,
    width: usize,
) -> Element<'a, Message> {
    let spacing = theme::spacing();
    let icon = match icon_opt {
        Some(icon) => widget::icon::icon(icon.clone())
            .size(ICON_SIZE_CARD)
            .apply(widget::container),
        None => widget::space()
            .width(ICON_SIZE_CARD)
            .height(ICON_SIZE_CARD)
            .apply(widget::container),
    }
    .padding(spacing.space_xxs)
    .class(theme::Container::Card);

    let column = widget::column::with_children([
        widget::text::heading(&info.name)
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
        widget::text::body(&info.summary)
            .ellipsize(Ellipsize::End(EllipsizeHeightLimit::Lines(1)))
            .into(),
        footer
            .apply(widget::container)
            .height(32.0)
            .align_y(Alignment::Center)
            .into(),
    ]);

    widget::row![icon, column]
        .align_y(Alignment::Center)
        .spacing(spacing.space_xs)
        .width(width as u32)
        .into()
}

impl Package {
    fn package_card_view<'a>(
        &'a self,
        controls: Vec<Element<'a, Message>>,
        width: usize,
    ) -> Element<'a, Message> {
        let controls = widget::row::with_children(controls)
            .spacing(theme::spacing().space_xxs)
            .into();
        card_view(&self.info, Some(&self.icon), controls, width)
    }
}

impl App {
    fn loading_indicator(&self, text: &str) -> Element<'_, Message> {
        widget::column::with_capacity(2)
            .push(widget::indeterminate_circular())
            .push(widget::text(text.to_string()))
            .spacing(theme::spacing().space_s)
            .align_x(Alignment::Center)
            .apply(widget::container)
            .center(Length::Fill)
            .into()
    }

    fn has_category_results_for_page(&self, nav_page: NavPage) -> bool {
        self.category_results
            .as_ref()
            .is_some_and(|(cats, results)| {
                !results.is_empty()
                    && nav_page
                        .categories()
                        .is_some_and(|page_cats| std::ptr::eq(*cats, page_cats))
            })
    }

    fn is_waiting_refresh(
        &self,
        backend_name: BackendName,
        source_id: &str,
        package_id: &AppId,
    ) -> bool {
        self.waiting_installed
            .iter()
            .chain(self.waiting_updates.iter())
            .any(|(b, s, p)| *b == backend_name && s == source_id && p == package_id)
    }

    fn progress_opt(
        &self,
        backend_name: BackendName,
        source_id: &str,
        package_id: &AppId,
    ) -> Option<f32> {
        self.pending_operations.values().find_map(|(op, progress)| {
            (op.backend_name == backend_name
                && op.infos.iter().any(|info| info.source_id == *source_id)
                && op.package_ids.iter().any(|pid| pid == package_id))
            .then_some(*progress)
        })
    }

    fn selected_buttons(
        &self,
        selected_backend_name: BackendName,
        selected_id: &AppId,
        selected_info: &Arc<AppInfo>,
        addon: bool,
    ) -> Vec<Element<'_, Message>> {
        //TODO: more efficient checks
        if let Some(progress) =
            self.progress_opt(selected_backend_name, &selected_info.source_id, selected_id)
        {
            return vec![
                widget::determinate_linear(progress)
                    .width(Length::Fill)
                    .into(),
            ];
        }

        // Do not show buttons while waiting for refresh
        if self.is_waiting_refresh(selected_backend_name, &selected_info.source_id, selected_id) {
            return Vec::new();
        }

        if !self.is_installed(selected_backend_name, selected_id, selected_info) {
            return vec![
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
            ];
        }

        let mut buttons = Vec::with_capacity(3);
        let applet_provide = AppProvide::Id("com.system76.CosmicApplet".to_string());
        let update_opt = self
            .updates
            .iter()
            .flatten()
            .find_map(|(backend_name, package)| {
                (*backend_name == selected_backend_name
                    && package.info.source_id == selected_info.source_id
                    && &package.id == selected_id)
                    .then(|| {
                        Message::Operation(
                            OperationKind::Update,
                            *backend_name,
                            package.id.clone(),
                            package.info.clone(),
                        )
                    })
            });

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

        buttons
    }

    pub fn selected_sources(
        &self,
        backend_name: BackendName,
        id: &AppId,
        info: &AppInfo,
    ) -> Vec<SelectedSource> {
        if let Some(entries) = self.apps.get(id) {
            entries
                .iter()
                .map(|e| SelectedSource::new(e.backend_name, &e.info, e.installed))
                .collect()
        } else {
            //TODO: warning?
            let installed = self.is_installed(backend_name, id, info);
            vec![SelectedSource::new(backend_name, info, installed)]
        }
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
                if appstream_cache.source_id == info.source_id
                    && let Some(ids) = appstream_cache.addons.get(id)
                {
                    for id in ids {
                        if let Some(info) = appstream_cache.infos.get(id) {
                            addons.push((id.clone(), info.clone()));
                        }
                    }
                }
            }
        }
        addons.sort_unstable_by(|a, b| {
            b.1.monthly_downloads
                .cmp(&a.1.monthly_downloads)
                .then_with(|| LANGUAGE_SORTER.compare(&a.1.name, &b.1.name))
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

    /// Full-screen overlay showing an enlarged screenshot with prev/next navigation.
    /// `dialog()` calls this when `Selected::screenshot_gallery` is set. Reuses the
    /// handles already downloaded into `Selected::screenshot_images`.
    pub fn screenshot_gallery_view(&self) -> Option<Element<'_, Message>> {
        let selected = self.selected_opt.as_ref()?;
        if !selected.screenshot_gallery {
            return None;
        }
        let screenshots = &selected.info.screenshots;
        let screenshot = screenshots.get(selected.screenshot_shown)?;
        let spacing = theme::active().cosmic().spacing;
        let space_m = Length::Fixed(spacing.space_m.into());

        let mut column = widget::column::with_capacity(2).spacing(spacing.space_s);

        // Header row: caption + close button
        {
            let mut row = widget::row::with_capacity(3).align_y(Alignment::Center);
            row = row.push(widget::space::horizontal());
            if !screenshot.caption.is_empty() {
                row = row.push(widget::text::heading(&screenshot.caption));
            }
            row = row.push(widget::space::horizontal());
            row = row.push(
                widget::button::icon(widget::icon::from_name("window-close-symbolic").size(16))
                    .class(theme::Button::Standard)
                    .on_press(Message::ScreenshotGallery(false)),
            );
            column = column.push(row);
        }

        // Navigation + image row: prev button | image | next button
        {
            let has_multiple = screenshots.len() > 1;
            let mut row = widget::row::with_capacity(5).align_y(Alignment::Center);
            row = row.push(widget::space::horizontal().width(space_m));
            if has_multiple {
                row = row.push(
                    widget::button::icon(widget::icon::from_name("go-previous-symbolic").size(16))
                        .padding(spacing.space_xs)
                        .class(theme::Button::Standard)
                        .on_press(Message::ScreenshotGalleryPrev),
                );
            }
            let image_element: Element<'_, Message> =
                if let Some(image) = selected.screenshot_images.get(&selected.screenshot_shown) {
                    // Scale up to fill the available space, preserving aspect ratio
                    widget::container(
                        widget::image(image.clone())
                            .width(Length::Fill)
                            .height(Length::Fill),
                    )
                    .center_x(Length::Fill)
                    .center_y(Length::Fill)
                    .into()
                } else {
                    widget::container(widget::text::body(fl!("loading")))
                        .center_x(Length::Fill)
                        .center_y(Length::Fill)
                        .into()
                };
            row = row.push(image_element);
            if has_multiple {
                row = row.push(
                    widget::button::icon(widget::icon::from_name("go-next-symbolic").size(16))
                        .padding(spacing.space_xs)
                        .class(theme::Button::Standard)
                        .on_press(Message::ScreenshotGalleryNext),
                );
            }
            row = row.push(widget::space::horizontal().width(space_m));
            column = column.push(row.height(Length::Fill));
        }

        Some(
            widget::container(column.padding(spacing.space_m))
                .width(Length::Fill)
                .height(Length::Fill)
                .class(theme::Container::custom(|theme| {
                    let cosmic = theme.cosmic();
                    let mut bg = cosmic.bg_color();
                    bg.alpha = 0.9;
                    widget::container::Style {
                        background: Some(Color::from(bg).into()),
                        ..Default::default()
                    }
                }))
                .into(),
        )
    }

    pub fn calc_grid_width(&self, size: Size) -> usize {
        (size.width as usize).saturating_sub(2 * self.side_padding() as usize)
    }

    pub fn view_responsive(&self, size: Size) -> Element<'_, Message> {
        self.size.set(Some(size));
        let grid_width = self.calc_grid_width(size);

        if let Some(selected) = &self.selected_opt {
            return self.view_selected(selected, grid_width);
        }

        if let Some((input, results)) = &self.search_results {
            return self.view_search_results(input, results, grid_width);
        }

        let nav_page = self
            .nav_model
            .active_data::<NavPage>()
            .copied()
            .unwrap_or_default();

        match nav_page {
            NavPage::Explore => self.view_explore_page(size, grid_width),
            NavPage::Installed => self.view_installed_page(grid_width),
            NavPage::Updates => self.view_updates_page(size, grid_width),
            _ => self.view_category_page(nav_page, size, grid_width),
        }
    }

    fn view_selected<'a>(
        &'a self,
        selected: &'a Selected,
        grid_width: usize,
    ) -> Element<'a, Message> {
        let spacing = theme::spacing();
        let selected_source = selected.sources.iter().position(|source| {
            source.backend_name == selected.backend_name
                && source.source_id == selected.info.source_id
        });

        let mut column = widget::column::with_capacity(8)
            .spacing(spacing.space_m)
            .width(Length::Fill);

        let buttons =
            self.selected_buttons(selected.backend_name, &selected.id, &selected.info, false);
        column = column.push(
            widget::row::with_children([
                match &selected.icon_opt {
                    Some(icon) => widget::icon::icon(icon.clone())
                        .size(ICON_SIZE_DETAILS)
                        .into(),
                    None => widget::space().width(ICON_SIZE_DETAILS).into(),
                },
                widget::column::with_children([
                    widget::text::title2(&selected.info.name).into(),
                    widget::text(&selected.info.summary).into(),
                    widget::space::vertical().height(spacing.space_s).into(),
                    widget::row::with_children(buttons)
                        .spacing(spacing.space_xs)
                        .into(),
                ])
                .into(),
            ])
            .align_y(Alignment::Center)
            .spacing(spacing.space_m),
        );

        let sources_widget = widget::column::with_children([if selected.sources.len() == 1 {
            widget::text(selected.sources[0].as_ref()).into()
        } else {
            widget::dropdown(&selected.sources, selected_source, Message::SelectedSource).into()
        }])
        .align_x(Alignment::Center)
        .width(Length::Fill);

        let developers_widget = widget::column::with_children([
            if selected.info.developer_name.is_empty() {
                widget::text::heading(fl!("app-developers", app = selected.info.name.as_str()))
                    .center()
                    .into()
            } else {
                widget::text::heading(&selected.info.developer_name)
                    .center()
                    .into()
            },
            widget::text::body(fl!("developer")).center().into(),
        ])
        .align_x(Alignment::Center)
        .width(Length::Fill);

        let downloads_widget = (selected.info.monthly_downloads > 0).then(|| {
            widget::column::with_children([
                widget::text::heading(selected.info.monthly_downloads.to_string())
                    .center()
                    .into(),
                //TODO: description of what this means?
                widget::text::body(fl!("monthly-downloads")).center().into(),
            ])
            .align_x(Alignment::Center)
            .width(Length::Fill)
        });

        if grid_width < 416 {
            let size = 4 + if downloads_widget.is_some() { 3 } else { 0 };
            let downloads_widget_space = downloads_widget
                .is_some()
                .then_some(widget::divider::horizontal::default());
            column = column.push(
                widget::column::with_capacity(size)
                    .push(widget::divider::horizontal::default())
                    .push(sources_widget)
                    .push(widget::divider::horizontal::default())
                    .push(developers_widget)
                    .push(widget::divider::horizontal::default())
                    .push_maybe(downloads_widget)
                    .push_maybe(downloads_widget_space)
                    .spacing(spacing.space_xxs),
            );
        } else {
            let row_size = 4 + if downloads_widget.is_some() { 2 } else { 0 };
            let downloads_widget_space = downloads_widget
                .is_some()
                .then_some(widget::divider::vertical::default().height(32));
            column = column.push(
                widget::column::with_children([
                    widget::divider::horizontal::default().into(),
                    widget::row::with_capacity(row_size)
                        .push(sources_widget)
                        .push(widget::divider::vertical::default().height(32))
                        .push(developers_widget)
                        .push_maybe(downloads_widget_space)
                        .push_maybe(downloads_widget)
                        .align_y(Alignment::Center)
                        .into(),
                    widget::divider::horizontal::default().into(),
                ])
                .spacing(spacing.space_xxs),
            );
        }
        //TODO: proper image scroller
        if let Some(screenshot) = selected.info.screenshots.get(selected.screenshot_shown) {
            let image_height = Length::Fixed(320.0);
            let has_multiple = selected.info.screenshots.len() > 1;
            let mut row = widget::row::with_capacity(3).align_y(Alignment::Center);
            if has_multiple {
                let mut button =
                    widget::button::icon(widget::icon::from_name("go-previous-symbolic").size(16));
                let index = selected
                    .screenshot_shown
                    .checked_sub(1)
                    .unwrap_or_else(|| selected.info.screenshots.len().saturating_sub(1));
                if index != selected.screenshot_shown {
                    button = button.on_press(Message::SelectedScreenshotShown(index));
                }
                row = row.push(button);
            }
            let image_element =
                if let Some(image) = selected.screenshot_images.get(&selected.screenshot_shown) {
                    widget::container(widget::image(image.clone()))
                        .center_x(Length::Fill)
                        .center_y(image_height)
                        .apply(widget::mouse_area)
                        .on_press(Message::ScreenshotGallery(true))
                        .into()
                } else {
                    widget::space::horizontal().height(image_height).into()
                };
            row = row.push(
                widget::column::with_children([
                    image_element,
                    widget::text::caption(&screenshot.caption).center().into(),
                ])
                .align_x(Alignment::Center),
            );
            if has_multiple {
                let mut button =
                    widget::button::icon(widget::icon::from_name("go-next-symbolic").size(16));
                let index = if selected.screenshot_shown + 1 == selected.info.screenshots.len() {
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
            let mut addon_col = widget::column::with_capacity(2).spacing(spacing.space_xxxs);
            addon_col = addon_col.push(widget::text::title4(fl!("addons")));
            let mut list = widget::list_column::with_capacity(selected.addons.len())
                .list_item_padding([spacing.space_xxs, 0])
                .style(theme::Container::Transparent);
            let addon_cnt = selected.addons.len();
            let take = if selected.addons_view_more {
                addon_cnt
            } else {
                4
            };
            for (addon_id, addon_info) in selected.addons.iter().take(take) {
                let buttons =
                    self.selected_buttons(selected.backend_name, addon_id, addon_info, true);
                list = list.add(
                    widget::settings::item::builder(&addon_info.name)
                        .description(&addon_info.summary)
                        .control(widget::row::with_children(buttons).spacing(spacing.space_xs)),
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
            let mut release_col = widget::column::with_capacity(2).spacing(spacing.space_xxxs);
            release_col = release_col.push(widget::text::title4(fl!(
                "version",
                version = release.version.as_str()
            )));
            if let Some(timestamp) = release.timestamp
                && let Some(utc) = chrono::DateTime::<chrono::Utc>::from_timestamp(timestamp, 0)
            {
                let local = chrono::DateTime::<chrono::Local>::from(utc);
                release_col = release_col.push(widget::text::body(format!(
                    "{}",
                    local.format("%b %-d, %-Y")
                )));
            }
            if let Some(description) = &release.description {
                release_col = release_col.push(widget::text::body(description));
            }
            column = column.push(release_col);
        }

        if let Some(license) = &selected.info.license_opt {
            let mut license_col = widget::column::with_capacity(2).spacing(spacing.space_xxxs);
            license_col = license_col.push(widget::text::title4(fl!("licenses")));
            if let Ok(expr) = spdx::Expression::parse_mode(license, spdx::ParseMode::LAX) {
                for item in expr.requirements() {
                    match &item.req.license {
                        spdx::LicenseItem::Spdx { id, .. } => {
                            license_col = license_col.push(widget::text::body(id.full_name));
                        }
                        spdx::LicenseItem::Other { lic_ref, .. } => {
                            license_col = if let Some((_, url)) = lic_ref.split_once('=') {
                                license_col.push(
                                    widget::button::link(fl!("proprietary"))
                                        .on_press(Message::LaunchUrl(url.to_string()))
                                        .padding(0),
                                )
                            } else {
                                license_col.push(widget::text::body(fl!("proprietary")))
                            };
                        }
                    }
                }
            } else {
                license_col = license_col.push(widget::text::body(license));
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
                    .push(widget::column::with_children(url_items).spacing(spacing.space_xxxs));
            } else {
                column = column.push(
                    widget::row::with_children(url_items)
                        .spacing(spacing.space_s)
                        .align_y(Alignment::Center),
                );
            }
        }

        column.into()
    }

    fn view_search_results<'a>(
        &'a self,
        input: &str,
        results: &'a [SearchResult],
        grid_width: usize,
    ) -> Element<'a, Message> {
        let spacing = theme::spacing();
        //TODO: paging or dynamic load
        let results_len = results.len().min(MAX_RESULTS);

        let mut column = widget::column::with_capacity(2)
            .spacing(spacing.space_xxs)
            .width(Length::Fill);
        //TODO: back button?
        if results.is_empty() {
            column = column.push(widget::text::body(fl!("no-results", search = input)));
        }
        column = column.push(SearchResult::grid_view(
            &results[..results_len],
            grid_width,
            Message::SelectSearchResult,
        ));
        column.into()
    }

    fn view_explore_page(&self, size: Size, grid_width: usize) -> Element<'_, Message> {
        let spacing = theme::spacing();
        if let Some(explore_page) = self.explore_page_opt {
            let mut column = widget::column::with_capacity(2)
                .spacing(spacing.space_xxs)
                .width(Length::Fill);
            column = column.push(widget::text::title4(explore_page.title()));
            //TODO: ensure explore_page matches
            if let Some(results) = self.explore_results.get(&explore_page) {
                //TODO: paging or dynamic load
                let results_len = results.len().min(MAX_RESULTS);
                if results.is_empty() {
                    //TODO: no results message?
                }
                column = column.push(SearchResult::grid_view(
                    &results[..results_len],
                    grid_width,
                    move |result_i| Message::SelectExploreResult(explore_page, result_i),
                ));
            } else {
                // Show loading indicator
                return column
                    .push(self.loading_indicator(&fl!("loading")))
                    .height(size.height)
                    .into();
            }

            column.into()
        } else if self.explore_results.is_empty() {
            // Show loading indicator if no results yet
            widget::container(self.loading_indicator(&fl!("loading")))
                .height(size.height)
                .into()
        } else {
            let explore_pages = ExplorePage::all();
            let mut column = widget::column::with_capacity(explore_pages.len())
                .spacing(spacing.space_xl)
                .width(Length::Fill);
            let GridMetrics { cols, .. } = GridMetrics::new(grid_width);
            let max_results = Self::explore_section_max_results(cols);

            for explore_page in explore_pages.iter() {
                //TODO: ensure explore_page matches
                if let Some(results) = self.explore_results.get(explore_page)
                    && !results.is_empty()
                {
                    let results_len = results.len().min(max_results);

                    column = column.push(
                        widget::column::with_children([
                            widget::row::with_children([
                                widget::text::title4(explore_page.title())
                                    .apply(widget::mouse_area)
                                    .on_press(Message::ExplorePage(Some(*explore_page)))
                                    .into(),
                                icon_cache_handle("go-next-symbolic", 16)
                                    .apply(widget::button::icon)
                                    .on_press(Message::ExplorePage(Some(*explore_page)))
                                    .into(),
                            ])
                            .align_y(Alignment::Center)
                            .into(),
                            SearchResult::grid_view(
                                &results[..results_len],
                                grid_width,
                                |result_i| Message::SelectExploreResult(*explore_page, result_i),
                            ),
                        ])
                        .spacing(spacing.space_xxs),
                    );
                }
            }
            column.into()
        }
    }

    fn view_installed_page(&self, grid_width: usize) -> Element<'_, Message> {
        let spacing = theme::spacing();
        let mut column = widget::column::with_capacity(3)
            .spacing(spacing.space_xxs)
            .width(Length::Fill);
        column = column.push(widget::text::title2(NavPage::Installed.title()));

        if let Some(installed) = &self.installed_results {
            if installed.is_empty() {
                column = column.push(widget::text(fl!("no-installed-applications")));
            }

            let metrics = GridMetrics::new(grid_width);
            let items = installed.iter().enumerate().map(|(i, result)| {
                let button: Element<_> = if let Some(desktop_id) = result.info.desktop_ids.first() {
                    widget::button::standard(fl!("open"))
                        .on_press(Message::OpenDesktopId(desktop_id.clone()))
                        .into()
                } else {
                    widget::space().into()
                };
                widget::mouse_area(card_view(
                    &result.info,
                    result.icon_opt.as_ref(),
                    button,
                    metrics.item_width,
                ))
                .on_press(Message::SelectInstalled(i))
                .into()
            });
            column = column.push(metrics.build_grid(items));
        } else {
            //TODO: loading message?
        }
        column.into()
    }

    fn view_updates_page(&self, size: Size, grid_width: usize) -> Element<'_, Message> {
        let spacing = theme::spacing();
        let Some(updates) = &self.updates else {
            return widget::column::with_capacity(2)
                .spacing(spacing.space_xxs)
                .width(Length::Fill)
                .height(size.height)
                .push(widget::text::title2(NavPage::Updates.title()))
                .push(self.loading_indicator(&fl!("checking-for-updates")))
                .into();
        };

        let mut column = widget::column::with_capacity(3)
            .spacing(spacing.space_xxs)
            .width(Length::Fill);

        if updates.is_empty() {
            column = column
                .push(widget::text::title2(NavPage::Updates.title()))
                .push(
                    widget::column::with_capacity(2)
                        .spacing(spacing.space_s)
                        .padding([spacing.space_l, 0])
                        .width(Length::Fill)
                        .align_x(Alignment::Center)
                        .push(widget::text::body(fl!("no-updates")))
                        .push(
                            widget::button::standard(fl!("check-for-updates"))
                                .on_press(Message::CheckUpdates),
                        ),
                );
        } else {
            column = column.push(
                widget::flex_row(vec![
                    widget::text::title2(NavPage::Updates.title()).into(),
                    widget::space::horizontal().into(),
                    widget::row::with_capacity(2)
                        .align_y(Alignment::Center)
                        .spacing(spacing.space_xxs)
                        .push(
                            widget::button::standard(fl!("check-for-updates"))
                                .on_press(Message::CheckUpdates),
                        )
                        .push(
                            widget::button::standard(fl!("update-all"))
                                .on_press(Message::UpdateAll),
                        )
                        .into(),
                ])
                .align_items(Alignment::Center),
            );
        }

        let metrics = GridMetrics::new(grid_width);
        let items = updates
            .iter()
            .enumerate()
            .map(|(updates_i, (backend_name, package))| {
                let waiting_refresh =
                    self.is_waiting_refresh(*backend_name, &package.info.source_id, &package.id);
                let progress_opt =
                    self.progress_opt(*backend_name, &package.info.source_id, &package.id);
                let controls = if let Some(progress) = progress_opt {
                    vec![
                        widget::determinate_linear(progress)
                            .width(Length::Fill)
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
                        widget::icon::from_name("help-info-symbolic")
                            .apply(widget::button::icon)
                            .class(theme::Button::Standard)
                            .on_press(Message::ToggleContextPage(ContextPage::ReleaseNotes(
                                updates_i,
                                package.info.name.clone(),
                            )))
                            .into(),
                    ]
                };

                package
                    .package_card_view(controls, metrics.item_width)
                    .apply(widget::mouse_area)
                    .on_press(Message::SelectUpdates(updates_i))
                    .into()
            });

        column = column.push(metrics.build_grid(items));
        column.into()
    }

    fn view_category_page(
        &self,
        nav_page: NavPage,
        size: Size,
        grid_width: usize,
    ) -> Element<'_, Message> {
        let spacing = theme::spacing();
        // Show loading indicator when no results for current page
        if !self.has_category_results_for_page(nav_page) {
            return widget::column::with_capacity(2)
                .spacing(spacing.space_xxs)
                .width(Length::Fill)
                .height(Length::Fixed(size.height))
                .push(widget::text::title2(nav_page.title()))
                .push(self.loading_indicator(&fl!("loading")))
                .into();
        }

        let mut column = widget::column::with_capacity(3)
            .spacing(spacing.space_xxs)
            .width(Length::Fill);
        column = column.push(widget::text::title2(nav_page.title()));

        if matches!(nav_page, NavPage::Applets) {
            let sources = self.sources();
            if !sources.is_empty()
                && sources.iter().any(|source| {
                    matches!(source.kind, SourceKind::Recommended { enabled: false, .. })
                })
            {
                column = column.push(
                    widget::column::with_children([
                        widget::space::vertical().height(spacing.space_m).into(),
                        widget::text(fl!("enable-flathub-cosmic")).into(),
                        widget::space::vertical().height(spacing.space_m).into(),
                        widget::button::standard(fl!("manage-repositories"))
                            .on_press(Message::ToggleContextPage(ContextPage::Repositories))
                            .into(),
                        widget::space::vertical().height(spacing.space_l).into(),
                    ])
                    .align_x(Alignment::Center)
                    .width(Length::Fill),
                );
            }
        }
        //TODO: ensure category matches?
        if let Some((_, results)) = &self.category_results {
            //TODO: paging or dynamic load
            let results_len = results.len().min(MAX_RESULTS);
            if results.is_empty() {
                //TODO: no results message?
            }

            column = column.push(SearchResult::grid_view(
                &results[..results_len],
                grid_width,
                Message::SelectCategoryResult,
            ));
        }

        column.into()
    }
}
